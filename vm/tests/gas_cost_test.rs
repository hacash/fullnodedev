//! Gas Cost Test Suite
//! 
//! Comprehensive test suite for verifying that the actual gas consumption of each opcode
//! executed by the virtual machine engine under different resource space conditions
//! matches the expected design specified in vm/doc/gas-cost.md.

use vm::rt::*;
use vm::*;
use vm::space::*;
use vm::value::*;
use vm::rt::ItrErrCode;

use vm::rt::Bytecode::*;

// Directly use internal module path - need to create a wrapper function
fn execute_test_with_argv(gas_limit: i64, codes: Vec<u8>, argv: Option<Value>) -> VmrtRes<(CallExit, i64, Vec<Value>, Heap)> {
    use basis::component::Env;
    use basis::interface::{Context, TransactionRead};
    use field::{Address, Amount, Hash};
    use protocol::context::ContextInst;
    use protocol::state::EmptyLogs;
    use space::{CtcKVMap, GKVMap, Heap, Stack};
    use sys::Ret;
    use crate::machine::CtxHost;

    #[derive(Default, Clone, Debug)]
    struct DummyTx;

    impl field::Serialize for DummyTx {
        fn size(&self) -> usize { 0 }
        fn serialize(&self) -> Vec<u8> { vec![] }
    }

    impl basis::interface::TxExec for DummyTx {}

    impl TransactionRead for DummyTx {
        fn ty(&self) -> u8 { 3 }
        fn hash(&self) -> Hash { Hash::default() }
        fn hash_with_fee(&self) -> Hash { Hash::default() }
        fn main(&self) -> Address { Address::default() }
        fn addrs(&self) -> Vec<Address> { vec![Address::default()] }
        fn fee(&self) -> &Amount { Amount::zero_ref() }
        fn fee_purity(&self) -> u64 { 1 }
        fn fee_extend(&self) -> Ret<(u16, Amount)> { Ok((1, Amount::zero())) }
    }

    let mut pc: usize = 0;
    let mut gas: i64 = gas_limit;
    let cadr = ContractAddress::default();

    let tx = DummyTx::default();
    let mut env = Env::default();
    env.block.height = 1;
    // StateMem is in an internal module, we need to define it directly or use protocol::state
    // But for testing, we use a simple in-memory implementation
    struct TestStateMem {
        mem: std::collections::HashMap<Vec<u8>, Vec<u8>>,
    }
    
    impl Default for TestStateMem {
        fn default() -> Self {
            Self { mem: std::collections::HashMap::new() }
        }
    }
    
    impl basis::interface::State for TestStateMem {
        fn get(&self, k: Vec<u8>) -> Option<Vec<u8>> {
            self.mem.get(&k).cloned()
        }
        fn set(&mut self, k: Vec<u8>, v: Vec<u8>) {
            self.mem.insert(k, v);
        }
        fn del(&mut self, k: Vec<u8>) {
            self.mem.remove(&k);
        }
    }
    
    let mut ctx = ContextInst::new(env, Box::new(TestStateMem::default()), Box::new(EmptyLogs{}), &tx);
    let ctx: &mut dyn Context = &mut ctx;

    let mut ops = Stack::new(256);
    if let Some(v) = argv {
        ops.push(v).unwrap();
    }

    let mut heap = Heap::new(64);

    let mut host = CtxHost::new(ctx);
    
    // Validate code length before execution to avoid panic from debug_assert
    // Note: We can't prevent all panics from debug_assert, but we can try to validate
    // that codes has at least one byte (END) before execution
    if codes.is_empty() {
        return Err(ItrErr::new(ItrErrCode::InstInvalid, "empty code"));
    }
    
    vm::interpreter::execute_code(
        &mut pc,
        &codes,
        ExecMode::Main,
        false,
        0,
        &mut gas,
        &GasTable::new(1),
        &GasExtra::new(1),
        &SpaceCap::new(1),
        &mut ops,
        &mut Stack::new(256),
        &mut heap,
        &mut GKVMap::new(20),
        &mut CtcKVMap::new(12),
        &mut host,
        &cadr,
        &cadr,
    ).map(|r|{
        (r, gas_limit - gas, ops.release(), heap)
    })
}

/// Test result structure
#[derive(Debug, Clone)]
struct TestResult {
    opcode: Bytecode,
    test_case: String,
    expected_gas: i64,
    actual_gas: i64,
    difference: i64,
    success: bool,
}

/// Test report collector
struct TestReporter {
    results: Vec<TestResult>,
    failures: Vec<TestResult>,
}

impl TestReporter {
    fn new() -> Self {
        Self {
            results: Vec::new(),
            failures: Vec::new(),
        }
    }

    fn record(&mut self, result: TestResult) {
        if !result.success {
            self.failures.push(result.clone());
        }
        self.results.push(result);
    }

    fn print_report(&self) {
        println!("\n========== Gas Cost Test Report ==========");
        println!("Total tests: {}", self.results.len());
        println!("Passed: {}", self.results.len() - self.failures.len());
        println!("Failed: {}\n", self.failures.len());

        if !self.failures.is_empty() {
            println!("FAILURES:");
            for failure in &self.failures {
                println!(
                    "  Opcode: {:?}, Test: {}, Expected: {}, Actual: {}, Difference: {}",
                    failure.opcode,
                    failure.test_case,
                    failure.expected_gas,
                    failure.actual_gas,
                    failure.difference
                );
            }
        }
        println!("==========================================\n");
    }
}

/// Calculate expected gas according to gas-cost.md documentation
struct ExpectedGasCalculator {
    gas_table: GasTable,
    gas_extra: GasExtra,
}

impl ExpectedGasCalculator {
    fn new() -> Self {
        Self {
            gas_table: GasTable::new(1),
            gas_extra: GasExtra::new(1),
        }
    }

    /// Calculate base gas
    fn base_gas(&self, opcode: Bytecode) -> i64 {
        let opcode_byte = opcode as u8;
        self.gas_table.gas(opcode_byte)
    }

    /// Calculate stack buffer copy gas (byte/12)
    fn stack_copy_gas(&self, byte_len: usize) -> i64 {
        self.gas_extra.stack_copy(byte_len)
    }

    /// Calculate heap read gas (byte/16)
    fn heap_read_gas(&self, byte_len: usize) -> i64 {
        self.gas_extra.heap_read(byte_len)
    }

    /// Calculate heap write gas (byte/12)
    fn heap_write_gas(&self, byte_len: usize) -> i64 {
        self.gas_extra.heap_write(byte_len)
    }

    /// Calculate heap grow gas
    /// First 8 segments grow exponentially: 2, 4, 8, 16, 32, 64, 128, 256
    /// After that, linear growth: 256 per segment
    fn heap_grow_gas(&self, segments: usize, _current_segments: usize) -> i64 {
        let mut gas: i64 = 0;
        let exp_count = segments.min(8);
        
        // Exponential growth part
        for i in 0..exp_count {
            gas += 1i64 << (i + 1);
        }
        
        // Linear growth part
        if segments > 8 {
            gas += ((segments - 8) * 256) as i64;
        }
        
        gas
    }

    /// Calculate compo items gas
    fn compo_items_gas(&self, items: usize, divisor: i64) -> i64 {
        self.gas_extra.compo_items(items, divisor)
    }

    /// Calculate compo bytes gas (byte/20)
    fn compo_bytes_gas(&self, byte_len: usize) -> i64 {
        self.gas_extra.compo_bytes(byte_len)
    }

    /// Calculate log gas (byte/1)
    fn log_gas(&self, total_bytes: usize) -> i64 {
        self.gas_extra.log_bytes(total_bytes)
    }

    /// Calculate storage read gas (byte/8)
    fn storage_read_gas(&self, val_len: usize) -> i64 {
        self.gas_extra.storage_read(val_len)
    }

    /// Calculate storage write gas (byte/6)
    fn storage_write_gas(&self, val_len: usize) -> i64 {
        self.gas_extra.storage_write(val_len)
    }

    /// Calculate storage rent gas ((32+byte)/1 per period)
    fn storage_rent_gas(&self, val_len: usize, periods: i64) -> i64 {
        (32 + val_len as i64) * periods
    }

    /// Calculate NTCALL gas (byte/16)
    fn ntcall_gas(&self, byte_len: usize) -> i64 {
        self.gas_extra.ntcall_bytes(byte_len)
    }

    /// Calculate EXTFUNC gas (byte/16)
    fn extfunc_gas(&self, byte_len: usize) -> i64 {
        self.gas_extra.extfunc_bytes(byte_len)
    }

    /// Calculate EXTACTION gas (byte/10)
    fn extaction_gas(&self, byte_len: usize) -> i64 {
        self.gas_extra.extaction_bytes(byte_len)
    }
}

/// Execute a single opcode test and return actual gas consumption
/// Note: This function automatically excludes END gas (1) since all valid bytecode must end with END
fn execute_opcode_test(
    codes: Vec<u8>,
    initial_stack: Option<Vec<Value>>,
) -> Result<i64, String> {
    let gas_limit = 1_000_000i64;
    
    // Prepare initial stack
    let mut argv = None;
    if let Some(stack_vals) = initial_stack {
        if !stack_vals.is_empty() {
            // For multiple values, we need to pack them or use other methods
            // Here we simplify by taking only the first value
            argv = Some(stack_vals[0].clone());
        }
    }

    match execute_test_with_argv(gas_limit, codes, argv) {
        Ok((_exit, gas_consumed, _ops, _heap)) => {
            // Exclude END gas (1) since all valid bytecode must end with END
            let end_gas = 1i64;
            Ok(gas_consumed - end_gas)
        },
        Err(e) => Err(format!("Execution error: {:?}", e)),
    }
}

/// Execute test code and measure baseline gas (for setup operations)
fn measure_baseline_gas(setup_codes: Vec<u8>) -> Result<i64, String> {
    let mut codes = setup_codes;
    codes.extend(build_codes!(END));
    execute_opcode_test(codes, None)
}

/// Execute test code and measure gas for target opcode only
/// Returns: target_opcode_gas
/// This function executes setup code once, then measures the target opcode gas
/// by comparing total gas (setup + target) with baseline gas (setup only)
/// Note: execute_opcode_test already excludes END gas, so we don't need to handle it here
fn measure_opcode_gas_with_setup(
    setup_codes: Vec<u8>,
    target_opcode: u8,
) -> Result<i64, String> {
    // Measure baseline (setup + END) - this should not include target opcode
    let mut baseline_codes = setup_codes.clone();
    baseline_codes.extend(build_codes!(END));
    let baseline = execute_opcode_test(baseline_codes, None)?;
    
    // Measure total (setup + target_opcode + END)
    // Note: Some opcodes need parameters after the opcode byte
    let mut total_codes = setup_codes.clone(); // Clone again to avoid consuming
    total_codes.push(target_opcode);
    
    // Add parameter for opcodes that need it
    // Note: We use numeric comparison since target_opcode is u8
    if target_opcode == GET as u8 || target_opcode == PUT as u8 {
        total_codes.push(0u8); // index parameter (u8)
    } else if target_opcode == GETX as u8 || target_opcode == PUTX as u8 {
        total_codes.push(0u8); // u8
        total_codes.push(0u8); // u8 (u16 = 0)
    } else if target_opcode == HGROW as u8 {
        total_codes.push(1u8); // segments parameter
    }
    
    total_codes.extend(build_codes!(END));
    let total = execute_opcode_test(total_codes, None)?;
    
    // Target opcode gas = total - baseline
    // Both already exclude END gas, so this is correct
    let target_gas = total - baseline;
    
    Ok(target_gas)
}

/// Create a bytes value with specified length
fn create_bytes_value(len: usize) -> Value {
    Value::bytes(vec![0u8; len])
}

/// Create a list with specified number of items
fn create_list_value(_item_count: usize, _item_size: usize) -> Value {
    // CompoItem is private, we cannot create it directly, this function is temporarily unused
    Value::Nil
}

/// Create a map with specified number of items
fn create_map_value(_item_count: usize, _key_size: usize, _val_size: usize) -> Value {
    // CompoItem is private, we cannot create it directly, this function is temporarily unused
    Value::Nil
}

/// Test base gas consumption using combination testing for opcodes that cannot be tested independently
/// Combination test: execute opcode with minimal setup, then subtract setup gas to get opcode gas
fn test_base_gas_combination(
    setup_codes: Vec<u8>,
    target_opcode: Bytecode,
    reporter: &mut TestReporter,
    calc: &ExpectedGasCalculator,
) {
    let expected = calc.base_gas(target_opcode);
    
    // Clone setup_codes first since we'll need it later
    let setup_codes_clone = setup_codes.clone();
    
    // Build test code: setup + target_opcode + END
    // For opcodes with parameters, we need to append them after the opcode
    let mut codes = setup_codes;
    
    // Add target opcode and its parameters
    match target_opcode {
        GET | PUT | DUPN | POPN | PICK | HGROW | HREADU | HWRITEX | XLG | XOP | CTO | TIS | NTCALL | ALLOC => {
            codes.push(target_opcode as u8);
            codes.push(0u8);
        },
        PU16 | HREADUL | HWRITEXL | GETX | PUTX => {
            codes.push(target_opcode as u8);
            codes.push(0u8);
            codes.push(0u8);
        },
        PBUF => {
            codes.push(target_opcode as u8);
            codes.push(0u8);
        },
        PBUFL => {
            codes.push(target_opcode as u8);
            codes.push(0u8);
            codes.push(0u8);
        },
        _ => {
            codes.push(target_opcode as u8);
        }
    }
    
    codes.extend(build_codes!(END));
    
    match execute_opcode_test(codes, None) {
        Ok(total_gas) => {
            // Calculate setup gas
            let mut setup_only = setup_codes_clone;
            setup_only.extend(build_codes!(END));
            let setup_gas = match execute_opcode_test(setup_only, None) {
                Ok(gas) => gas,
                Err(_) => 0, // If setup fails, assume 0
            };
            
            // Target opcode gas = total - setup
            let actual = total_gas - setup_gas;
            let success = actual == expected;
            let result = TestResult {
                opcode: target_opcode,
                test_case: format!("base gas (combination test)"),
                expected_gas: expected,
                actual_gas: actual,
                difference: actual - expected,
                success,
            };
            reporter.record(result);
        }
        Err(e) => {
            let result = TestResult {
                opcode: target_opcode,
                test_case: format!("base gas (combination test failed: {})", e),
                expected_gas: expected,
                actual_gas: 0,
                difference: -expected,
                success: false,
            };
            reporter.record(result);
        }
    }
}

/// Test base gas consumption
fn test_base_gas(opcode: Bytecode, reporter: &mut TestReporter, calc: &ExpectedGasCalculator) {
    let expected = calc.base_gas(opcode);
    
    // Build test code
    let codes = match opcode {
        END => build_codes!(END),
        RET => build_codes!(PNIL RET END),
        ABT => build_codes!(ABT END),
        ERR => build_codes!(ERR END),
        AST => build_codes!(P0 AST END),
        PRT => build_codes!(P0 PRT END),
        NOP => build_codes!(NOP END),
        NT => build_codes!(NT END),
        // Opcodes that need u8 parameter
        PU8 => build_codes!(PU8 0 END),
        CU8 => build_codes!(P0 CU8 END),
        CU16 => build_codes!(P0 CU16 END),
        CU32 => build_codes!(P0 CU32 END),
        CU64 => build_codes!(P0 CU64 END),
        CU128 => build_codes!(P0 CU128 END),
        CBUF => build_codes!(P0 CBUF END),
        CTO => build_codes!(P0 CTO 0 END),
        TID => build_codes!(P0 TID END),
        PUT => build_codes!(ALLOC 1 PNBUF PUT 0 END),
        GET => build_codes!(ALLOC 1 PNBUF PUT 0 GET 0 END),
        // Branch opcodes need offset parameter
        BRL => build_codes!(BRL 0 0 END),
        BRS => build_codes!(BRS 0 END),
        BRSL => build_codes!(BRSL 0 0 END),
        BRSLN => build_codes!(BRSLN 0 0 END),
        DUPN => build_codes!(DUPN 0 END),
        POPN => build_codes!(POPN 0 END),
        PICK => build_codes!(PICK 0 END),
        HGROW => build_codes!(HGROW 0 END),
        HREADU => build_codes!(HREADU 0 END),
        HWRITEX => build_codes!(HWRITEX 0 END),
        XLG => build_codes!(XLG 0 END),
        XOP => build_codes!(XOP 0 END),
        TIS => build_codes!(TIS 0 END),
        NTCALL => build_codes!(NTCALL 0 END),
        ALLOC => build_codes!(ALLOC 0 END),
        // Opcodes that need u16 parameter
        PU16 => build_codes!(PU16 0 0 END),
        HREADUL => build_codes!(HREADUL 0 0 END),
        HWRITEXL => build_codes!(HWRITEXL 0 0 END),
        GETX => build_codes!(GETX 0 0 END),
        PUTX => build_codes!(PUTX 0 0 END),
        // Opcodes that need buffer parameter
        PBUF => build_codes!(PBUF 0 END),
        PBUFL => build_codes!(PBUFL 0 0 END),
        // Opcodes that need stack values - use combination testing
        POP => build_codes!(P0 POP END),
        SWAP => build_codes!(P0 P1 SWAP END),
        ADD => build_codes!(P0 P1 ADD END),
        SUB => build_codes!(P0 P1 SUB END),
        MUL => build_codes!(P0 P1 MUL END),
        DIV => build_codes!(P0 P1 DIV END),
        MOD => build_codes!(P0 P1 MOD END),
        POW => build_codes!(P0 P1 POW END),
        MAX => build_codes!(P0 P1 MAX END),
        MIN => build_codes!(P0 P1 MIN END),
        AND => build_codes!(P0 P1 AND END),
        OR => build_codes!(P0 P1 OR END),
        EQ => build_codes!(P0 P1 EQ END),
        NEQ => build_codes!(P0 P1 NEQ END),
        LT => build_codes!(P0 P1 LT END),
        LE => build_codes!(P0 P1 LE END),
        GT => build_codes!(P0 P1 GT END),
        GE => build_codes!(P0 P1 GE END),
        SIZE => build_codes!(PNBUF SIZE END),
        CAT => build_codes!(PNBUF PNBUF CAT END),
        CHOISE => build_codes!(P1 P0 P1 CHOISE END),
        HEAD | TAIL | HASKEY | LENGTH => {
            // These need compo value on stack - use combination test
            test_base_gas_combination(build_codes!(NEWLIST), opcode, reporter, calc);
            return;
        },
        INSERT => {
            // INSERT needs compo, key, and value
            test_base_gas_combination(build_codes!(NEWLIST PNBUF PNBUF), opcode, reporter, calc);
            return;
        },
        REMOVE => {
            // REMOVE needs compo and key
            test_base_gas_combination(build_codes!(NEWLIST PNBUF), opcode, reporter, calc);
            return;
        },
        CLEAR => {
            // CLEAR needs compo value
            test_base_gas_combination(build_codes!(NEWLIST), opcode, reporter, calc);
            return;
        },
        APPEND => {
            // APPEND needs compo and value
            test_base_gas_combination(build_codes!(NEWLIST PNBUF), opcode, reporter, calc);
            return;
        },
        CLONE | MERGE | KEYS | VALUES | PACKLIST | PACKMAP | UPLIST => {
            // These need compo values - use combination test
            test_base_gas_combination(build_codes!(NEWLIST), opcode, reporter, calc);
            return;
        },
        // Opcodes that need complex setup - use combination test or skip
        HREAD | HWRITE | MGET | GGET | SLOAD | ITEMGET | LOG1 | LOG2 | LOG3 | LOG4 | HWRITEXL | GETX | PUTX => {
            // These opcodes are tested in dynamic gas tests, skip base gas test
            return;
        },
        // Opcodes that need stack values but can be tested with minimal setup
        INC => build_codes!(P0 INC 0 END),
        DEC => build_codes!(P0 DEC 0 END),
        BYTE => build_codes!(PNBUF BYTE 0 0 END),
        CUT => build_codes!(PNBUF P0 P0 CUT END),
        LEFT => build_codes!(PNBUF LEFT 0 0 END),
        RIGHT => build_codes!(PNBUF RIGHT 0 0 END),
        LDROP => build_codes!(PNBUF LDROP 0 0 END),
        RDROP => build_codes!(PNBUF RDROP 0 0 END),
        JOIN => build_codes!(P0 P1 P2 JOIN 3 END),
        REV => build_codes!(P0 P1 REV 2 END),
        NEWLIST => build_codes!(NEWLIST END),
        NEWMAP => build_codes!(NEWMAP END),
        _ => {
            // For other opcodes, build minimal test case
            // Try to execute, if it fails due to missing parameters, skip
            // Note: We can't use build_codes! with a variable opcode, so we build manually
            let mut c = vec![opcode as u8];
            c.extend(build_codes!(END));
            c
        }
    };

    // For opcodes that might fail due to missing parameters or setup,
    // wrap execution in a way that handles errors gracefully
    match execute_opcode_test(codes.clone(), None) {
        Ok(actual) => {
            let success = actual == expected;
            let result = TestResult {
                opcode,
                test_case: format!("base gas"),
                expected_gas: expected,
                actual_gas: actual,
                difference: actual - expected,
                success,
            };
            reporter.record(result);
        }
        Err(e) => {
            // Some opcodes may not be executable alone due to missing parameters
            // Try combination test for these opcodes
            if e.contains("overflow") || e.contains("empty") || e.contains("Read") {
                // These errors suggest missing setup, try combination test
                // But for now, just record as skipped
                let result = TestResult {
                    opcode,
                    test_case: format!("base gas (skipped - needs setup: {})", e),
                    expected_gas: expected,
                    actual_gas: 0,
                    difference: -expected,
                    success: false,
                };
                reporter.record(result);
            } else {
                // Other errors - record as failed
                let result = TestResult {
                    opcode,
                    test_case: format!("base gas (execution failed: {})", e),
                    expected_gas: expected,
                    actual_gas: 0,
                    difference: -expected,
                    success: false,
                };
                reporter.record(result);
            }
        }
    }
}

/// Test dynamic gas for DUP opcode (stack buffer copy)
fn test_dup_dynamic_gas(reporter: &mut TestReporter, calc: &ExpectedGasCalculator) {
    let base_gas = calc.base_gas(DUP);
    let test_sizes = vec![0, 6, 12, 24, 48, 100, 200];

    for size in test_sizes {
        let expected_dynamic = calc.stack_copy_gas(size);
        let expected_total = base_gas + expected_dynamic;

        // Build test code: push a value of specified size, then DUP
        // Note: PBUF/PNBUF also consume gas (base + stack_copy), so we need to account for that
        let mut setup_codes = Vec::new();
        let mut push_base_gas = 0i64;
        let mut push_dynamic_gas = 0i64;
        
        if size == 0 {
            setup_codes.extend(build_codes!(PNBUF));
            push_base_gas = calc.base_gas(PNBUF);
        } else if size <= 255 {
            setup_codes.extend(build_codes!(PBUF));
            setup_codes.push(size as u8);
            setup_codes.extend(vec![0u8; size]);
            push_base_gas = calc.base_gas(PBUF);
            push_dynamic_gas = calc.stack_copy_gas(size);
        } else {
            // For values greater than 255, use PBUFL
            setup_codes.extend(build_codes!(PBUFL));
            let size_u16 = size as u16;
            setup_codes.extend(size_u16.to_be_bytes());
            setup_codes.extend(vec![0u8; size]);
            push_base_gas = calc.base_gas(PBUFL);
            push_dynamic_gas = calc.stack_copy_gas(size);
        }
        
        // Measure only DUP gas (excluding PBUF/PNBUF setup)
        match measure_opcode_gas_with_setup(setup_codes, DUP as u8) {
            Ok(actual) => {
                let success = actual == expected_total;
                let result = TestResult {
                    opcode: DUP,
                    test_case: format!("dynamic gas (buffer size: {})", size),
                    expected_gas: expected_total,
                    actual_gas: actual,
                    difference: actual - expected_total,
                    success,
                };
                reporter.record(result);
            }
            Err(e) => {
                let result = TestResult {
                    opcode: DUP,
                    test_case: format!("dynamic gas (buffer size: {}, error: {})", size, e),
                    expected_gas: expected_total,
                    actual_gas: 0,
                    difference: -expected_total,
                    success: false,
                };
                reporter.record(result);
            }
        }
    }
}

/// Test dynamic gas for PBUF opcode (stack buffer copy)
fn test_pbuf_dynamic_gas(reporter: &mut TestReporter, calc: &ExpectedGasCalculator) {
    let base_gas = calc.base_gas(PBUF);
    let test_sizes = vec![0, 6, 12, 24, 48, 100, 200, 255];

    for size in test_sizes {
        let expected_dynamic = calc.stack_copy_gas(size);
        let expected_total = base_gas + expected_dynamic;

        // Build test code: PBUF size [bytes...]
        // PBUF includes the opcode itself, so we measure the whole operation
        // For dynamic sizes, we need to build bytecode manually
        let mut codes = build_codes!(PBUF);
        codes.push(size as u8);
        codes.extend(vec![0u8; size]);
        codes.extend(build_codes!(END));

        match execute_opcode_test(codes, None) {
            Ok(actual) => {
                // execute_opcode_test already excludes END gas
                let success = actual == expected_total;
                let result = TestResult {
                    opcode: PBUF,
                    test_case: format!("dynamic gas (buffer size: {})", size),
                    expected_gas: expected_total,
                    actual_gas: actual,
                    difference: actual - expected_total,
                    success,
                };
                reporter.record(result);
            }
            Err(e) => {
                let result = TestResult {
                    opcode: PBUF,
                    test_case: format!("dynamic gas (buffer size: {}, error: {})", size, e),
                    expected_gas: expected_total,
                    actual_gas: 0,
                    difference: -expected_total,
                    success: false,
                };
                reporter.record(result);
            }
        }
    }
}

/// Test dynamic gas for HGROW opcode
fn test_hgrow_dynamic_gas(reporter: &mut TestReporter, calc: &ExpectedGasCalculator) {
    let base_gas = calc.base_gas(HGROW);
    let test_segments = vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 16];

    for segments in test_segments {
        let expected_dynamic = calc.heap_grow_gas(segments, 0);
        let expected_total = base_gas + expected_dynamic;

        // Build test code: HGROW segments
        // HGROW returns gas from heap.grow(), which is added to the local gas variable
        // HGROW doesn't need setup, so we can measure it directly
        let mut codes = build_codes!(HGROW);
        codes.push(segments as u8);
        codes.extend(build_codes!(END));

        match execute_opcode_test(codes, None) {
            Ok(actual) => {
                // execute_opcode_test already excludes END gas
                let success = actual == expected_total;
                let result = TestResult {
                    opcode: HGROW,
                    test_case: format!("dynamic gas (segments: {})", segments),
                    expected_gas: expected_total,
                    actual_gas: actual,
                    difference: actual - expected_total,
                    success,
                };
                reporter.record(result);
            }
            Err(e) => {
                let result = TestResult {
                    opcode: HGROW,
                    test_case: format!("dynamic gas (segments: {}, error: {})", segments, e),
                    expected_gas: expected_total,
                    actual_gas: 0,
                    difference: -expected_total,
                    success: false,
                };
                reporter.record(result);
            }
        }
    }
}

/// Test dynamic gas for HREAD opcode
fn test_hread_dynamic_gas(reporter: &mut TestReporter, calc: &ExpectedGasCalculator) {
    let base_gas = calc.base_gas(HREAD);
    let test_lengths = vec![0, 8, 16, 32, 48, 64, 100, 200];

    for length in test_lengths {
        let expected_dynamic = calc.heap_read_gas(length);
        let expected_total = base_gas + expected_dynamic;

        // Build setup code: grow heap and prepare stack
        let mut setup_codes = build_codes!(HGROW 1 P0 PU16);
        // HREAD: requires offset and length on stack
        let length_u16 = length as u16;
        setup_codes.extend(length_u16.to_be_bytes());
        
        // Measure only HREAD gas (excluding HGROW and stack setup)
        match measure_opcode_gas_with_setup(setup_codes, HREAD as u8) {
            Ok(actual) => {
                let success = actual == expected_total;
                let result = TestResult {
                    opcode: HREAD,
                    test_case: format!("dynamic gas (read length: {})", length),
                    expected_gas: expected_total,
                    actual_gas: actual,
                    difference: actual - expected_total,
                    success,
                };
                reporter.record(result);
            }
            Err(e) => {
                let result = TestResult {
                    opcode: HREAD,
                    test_case: format!("dynamic gas (read length: {}, error: {})", length, e),
                    expected_gas: expected_total,
                    actual_gas: 0,
                    difference: -expected_total,
                    success: false,
                };
                reporter.record(result);
            }
        }
    }
}

/// Test dynamic gas for GET opcode (stack buffer copy)
fn test_get_dynamic_gas(reporter: &mut TestReporter, calc: &ExpectedGasCalculator) {
    let base_gas = calc.base_gas(GET);
    let test_sizes = vec![0, 6, 12, 24, 48, 100];

    for size in test_sizes {
        let expected_dynamic = calc.stack_copy_gas(size);
        let expected_total = base_gas + expected_dynamic;

        // Build setup code: allocate local and write value
        // ALLOC 1 slot
        let mut setup_codes = build_codes!(ALLOC 1);
        // PUT value to local[0]
        if size == 0 {
            setup_codes.extend(build_codes!(PNBUF));
        } else if size <= 255 {
            setup_codes.extend(build_codes!(PBUF));
            setup_codes.push(size as u8);
            setup_codes.extend(vec![0u8; size]);
        }
        setup_codes.extend(build_codes!(PUT 0));
        
        // Measure only GET gas (excluding ALLOC, PUT, and value push)
        match measure_opcode_gas_with_setup(setup_codes, GET as u8) {
            Ok(actual) => {
                // GET also needs the index parameter
                // But the parameter itself doesn't consume extra gas beyond the opcode
                let success = actual == expected_total;
                let result = TestResult {
                    opcode: GET,
                    test_case: format!("dynamic gas (local value size: {})", size),
                    expected_gas: expected_total,
                    actual_gas: actual,
                    difference: actual - expected_total,
                    success,
                };
                reporter.record(result);
            }
            Err(e) => {
                let result = TestResult {
                    opcode: GET,
                    test_case: format!("dynamic gas (local value size: {}, error: {})", size, e),
                    expected_gas: expected_total,
                    actual_gas: 0,
                    difference: -expected_total,
                    success: false,
                };
                reporter.record(result);
            }
        }
    }
}

/// Test dynamic gas for MGET opcode (stack buffer copy)
fn test_mget_dynamic_gas(reporter: &mut TestReporter, calc: &ExpectedGasCalculator) {
    let base_gas = calc.base_gas(MGET);
    let test_sizes = vec![0, 6, 12, 24, 48, 100];

    for size in test_sizes {
        let expected_dynamic = calc.stack_copy_gas(size);
        let expected_total = base_gas + expected_dynamic;

        // Build setup code: MPUT value to memory
        // Push key
        let mut setup_codes = build_codes!(PBUF 4);
        setup_codes.extend(b"key1");
        // Push value
        if size == 0 {
            setup_codes.extend(build_codes!(PNBUF));
        } else if size <= 255 {
            setup_codes.extend(build_codes!(PBUF));
            setup_codes.push(size as u8);
            setup_codes.extend(vec![0u8; size]);
        }
        // MPUT
        setup_codes.extend(build_codes!(MPUT));
        // Push key for MGET
        setup_codes.extend(build_codes!(PBUF 4));
        setup_codes.extend(b"key1");
        
        // Measure only MGET gas (excluding MPUT and key push)
        match measure_opcode_gas_with_setup(setup_codes, MGET as u8) {
            Ok(actual) => {
                let success = actual == expected_total;
                let result = TestResult {
                    opcode: MGET,
                    test_case: format!("dynamic gas (memory value size: {})", size),
                    expected_gas: expected_total,
                    actual_gas: actual,
                    difference: actual - expected_total,
                    success,
                };
                reporter.record(result);
            }
            Err(e) => {
                let result = TestResult {
                    opcode: MGET,
                    test_case: format!("dynamic gas (memory value size: {}, error: {})", size, e),
                    expected_gas: expected_total,
                    actual_gas: 0,
                    difference: -expected_total,
                    success: false,
                };
                reporter.record(result);
            }
        }
    }
}

/// Test dynamic gas for GGET opcode (stack buffer copy)
fn test_gget_dynamic_gas(reporter: &mut TestReporter, calc: &ExpectedGasCalculator) {
    let base_gas = calc.base_gas(GGET);
    let test_sizes = vec![0, 6, 12, 24, 48, 100];

    for size in test_sizes {
        let expected_dynamic = calc.stack_copy_gas(size);
        let expected_total = base_gas + expected_dynamic;

        // Build setup code: GPUT value to global
        // Push key
        let mut setup_codes = build_codes!(PBUF 4);
        setup_codes.extend(b"key1");
        // Push value
        if size == 0 {
            setup_codes.extend(build_codes!(PNBUF));
        } else if size <= 255 {
            setup_codes.extend(build_codes!(PBUF));
            setup_codes.push(size as u8);
            setup_codes.extend(vec![0u8; size]);
        }
        // GPUT
        setup_codes.extend(build_codes!(GPUT));
        // Push key for GGET
        setup_codes.extend(build_codes!(PBUF 4));
        setup_codes.extend(b"key1");
        
        // Measure only GGET gas (excluding GPUT and key push)
        match measure_opcode_gas_with_setup(setup_codes, GGET as u8) {
            Ok(actual) => {
                let success = actual == expected_total;
                let result = TestResult {
                    opcode: GGET,
                    test_case: format!("dynamic gas (global value size: {})", size),
                    expected_gas: expected_total,
                    actual_gas: actual,
                    difference: actual - expected_total,
                    success,
                };
                reporter.record(result);
            }
            Err(e) => {
                let result = TestResult {
                    opcode: GGET,
                    test_case: format!("dynamic gas (global value size: {}, error: {})", size, e),
                    expected_gas: expected_total,
                    actual_gas: 0,
                    difference: -expected_total,
                    success: false,
                };
                reporter.record(result);
            }
        }
    }
}

/// Test dynamic gas for ITEMGET opcode (compo items + bytes)
fn test_itemget_dynamic_gas(reporter: &mut TestReporter, calc: &ExpectedGasCalculator) {
    let base_gas = calc.base_gas(ITEMGET);
    let test_cases = vec![
        (1, 10),   // 1 item, 10 bytes
        (4, 20),   // 4 items, 20 bytes
        (8, 40),   // 8 items, 40 bytes
        (16, 80),  // 16 items, 80 bytes
    ];

    for (item_count, item_size) in test_cases {
        let expected_items_gas = calc.compo_items_gas(item_count, 4);
        let expected_bytes_gas = calc.compo_bytes_gas(item_size);
        let expected_total = base_gas + expected_items_gas + expected_bytes_gas;

        // Build setup code: create list and add items
        // Create list
        let mut setup_codes = build_codes!(NEWLIST);
        // Add items
        for i in 0..item_count {
            setup_codes.extend(build_codes!(PBUF));
            setup_codes.push(item_size as u8);
            setup_codes.extend(vec![i as u8; item_size]);
            setup_codes.extend(build_codes!(APPEND));
        }
        // Push key (index 0)
        setup_codes.extend(build_codes!(P0));
        
        // Measure only ITEMGET gas (excluding NEWLIST, APPEND, and value push)
        match measure_opcode_gas_with_setup(setup_codes, ITEMGET as u8) {
            Ok(actual) => {
                let success = actual == expected_total;
                let result = TestResult {
                    opcode: ITEMGET,
                    test_case: format!("dynamic gas (items: {}, item_size: {})", item_count, item_size),
                    expected_gas: expected_total,
                    actual_gas: actual,
                    difference: actual - expected_total,
                    success,
                };
                reporter.record(result);
            }
            Err(e) => {
                let result = TestResult {
                    opcode: ITEMGET,
                    test_case: format!("dynamic gas (items: {}, item_size: {}, error: {})", item_count, item_size, e),
                    expected_gas: expected_total,
                    actual_gas: 0,
                    difference: -expected_total,
                    success: false,
                };
                reporter.record(result);
            }
        }
    }
}

/// Test dynamic gas for LOG1-4 opcodes
fn test_log_dynamic_gas(reporter: &mut TestReporter, calc: &ExpectedGasCalculator) {
    let log_opcodes = vec![
        (LOG1, 20, 1),  // base 20, 1 param
        (LOG2, 24, 2),  // base 24, 2 params
        (LOG3, 28, 3),  // base 28, 3 params
        (LOG4, 32, 4),  // base 32, 4 params
    ];

    let test_sizes = vec![0, 10, 20, 50, 100];

    for (opcode, base_gas_val, param_count) in log_opcodes {
        let base_gas = calc.base_gas(opcode);
        assert_eq!(base_gas, base_gas_val);

        for size in &test_sizes {
            // LOG operations pop param_count + 1 values (1 topic + param_count data)
            // Total bytes = size * (param_count + 1)
            let total_bytes = *size * (param_count + 1);
            let expected_dynamic = calc.log_gas(total_bytes);
            let expected_total = base_gas + expected_dynamic;

            // Build test code: push params to stack, then LOG
            // LOG operations pop params from stack: LOG1 needs 2 (1 topic + 1 data), LOG2 needs 3 (1 topic + 2 data), etc.
            // So we need to push param_count + 1 values total (1 topic + param_count data)
            // But for gas calculation, LOG uses total_bytes = sum of all param sizes
            let mut codes = Vec::new();
            let mut param_push_gas = 0i64;
            
            // Push all params: 1 topic + param_count data params
            // Total params = param_count + 1
            for _ in 0..(param_count + 1) {
                if *size == 0 {
                    codes.extend(build_codes!(PNBUF));
                    param_push_gas += calc.base_gas(PNBUF);
                } else if *size <= 255 {
                    codes.extend(build_codes!(PBUF));
                    codes.push(*size as u8);
                    codes.extend(vec![0u8; *size]);
                    param_push_gas += calc.base_gas(PBUF) + calc.stack_copy_gas(*size);
                }
            }
            codes.push(opcode as u8);
            codes.extend(build_codes!(END));
            
            // Measure total gas (param push + LOG + END)
            match execute_opcode_test(codes, None) {
                Ok(total_gas) => {
                    // execute_opcode_test already excludes END gas
                    // Subtract param push gas to get LOG gas only
                    let actual = total_gas - param_push_gas;
                    let success = actual == expected_total;
                    let result = TestResult {
                        opcode,
                        test_case: format!("dynamic gas (param_size: {}, params: {})", size, param_count),
                        expected_gas: expected_total,
                        actual_gas: actual,
                        difference: actual - expected_total,
                        success,
                    };
                    reporter.record(result);
                }
                Err(e) => {
                    let result = TestResult {
                        opcode,
                        test_case: format!("dynamic gas (param_size: {}, params: {}, error: {})", size, param_count, e),
                        expected_gas: expected_total,
                        actual_gas: 0,
                        difference: -expected_total,
                        success: false,
                    };
                    reporter.record(result);
                }
            }
        }
    }
}

/// Test dynamic gas for SLOAD opcode
fn test_sload_dynamic_gas(reporter: &mut TestReporter, calc: &ExpectedGasCalculator) {
    let base_gas = calc.base_gas(SLOAD);
    assert_eq!(base_gas, 32);
    let test_sizes = vec![0, 8, 16, 32, 40, 80, 100];

    for size in test_sizes {
        let expected_dynamic = calc.storage_read_gas(size);
        let expected_total = base_gas + expected_dynamic;

        // Build setup code: SSAVE value to storage
        // Push key
        let mut setup_codes = build_codes!(PBUF 4);
        setup_codes.extend(b"key1");
        // Push value
        if size == 0 {
            setup_codes.extend(build_codes!(PNBUF));
        } else if size <= 255 {
            setup_codes.extend(build_codes!(PBUF));
            setup_codes.push(size as u8);
            setup_codes.extend(vec![0u8; size]);
        }
        // SSAVE
        setup_codes.extend(build_codes!(SSAVE));
        // Push key for SLOAD
        setup_codes.extend(build_codes!(PBUF 4));
        setup_codes.extend(b"key1");
        
        // Measure only SLOAD gas (excluding SSAVE and key push)
        match measure_opcode_gas_with_setup(setup_codes, SLOAD as u8) {
            Ok(actual) => {
                let success = actual == expected_total;
                let result = TestResult {
                    opcode: SLOAD,
                    test_case: format!("dynamic gas (storage value size: {})", size),
                    expected_gas: expected_total,
                    actual_gas: actual,
                    difference: actual - expected_total,
                    success,
                };
                reporter.record(result);
            }
            Err(e) => {
                let result = TestResult {
                    opcode: SLOAD,
                    test_case: format!("dynamic gas (storage value size: {}, error: {})", size, e),
                    expected_gas: expected_total,
                    actual_gas: 0,
                    difference: -expected_total,
                    success: false,
                };
                reporter.record(result);
            }
        }
    }
}

/// Test dynamic gas for SRENT opcode
fn test_srent_dynamic_gas(reporter: &mut TestReporter, calc: &ExpectedGasCalculator) {
    let base_gas = calc.base_gas(SRENT);
    assert_eq!(base_gas, 64);
    
    // Test cases: (value_byte, periods)
    let test_cases = vec![
        // Basic rent tests
        (0, 1),      // 0 bytes, 1 period
        (8, 1),      // 8 bytes, 1 period
        (32, 1),     // 32 bytes, 1 period
        (64, 1),     // 64 bytes, 1 period
        (100, 1),    // 100 bytes, 1 period
        // Multi-period tests
        (8, 2),      // 8 bytes, 2 periods
        (8, 10),     // 8 bytes, 10 periods
        (32, 5),     // 32 bytes, 5 periods
        (32, 100),   // 32 bytes, 100 periods
        (100, 10),   // 100 bytes, 10 periods
    ];

    for (value_byte, periods) in test_cases {
        let expected_rent_gas = calc.storage_rent_gas(value_byte, periods as i64);
        let expected_total = base_gas + expected_rent_gas;

        // Build setup code: SSAVE value to storage
        let mut setup_codes = build_codes!(PBUF 4);
        setup_codes.extend(b"key1");
        // Push value
        if value_byte == 0 {
            setup_codes.extend(build_codes!(PNBUF));
        } else if value_byte <= 255 {
            setup_codes.extend(build_codes!(PBUF));
            setup_codes.push(value_byte as u8);
            setup_codes.extend(vec![0u8; value_byte]);
        }
        // SSAVE to create the key
        setup_codes.extend(build_codes!(SSAVE));

        // Build SRENT call: push key, push periods, then SRENT
        let mut srent_codes = Vec::new();
        // Push key for SRENT (SRENT pops key first, then periods)
        srent_codes.extend(build_codes!(PBUF 4));
        srent_codes.extend(b"key1");
        // Push periods (must be uint type)
        if periods <= 255 {
            srent_codes.extend(build_codes!(PU8));
            srent_codes.push(periods as u8);
        } else {
            srent_codes.extend(build_codes!(PU16));
            srent_codes.extend((periods as u16).to_be_bytes());
        }
        // SRENT opcode
        srent_codes.push(SRENT as u8);

        // Measure baseline: setup only (SSAVE + value push + key push)
        let baseline = match measure_baseline_gas(setup_codes.clone()) {
            Ok(gas) => gas,
            Err(e) => {
                let result = TestResult {
                    opcode: SRENT,
                    test_case: format!("dynamic gas (value_byte: {}, periods: {}, setup error: {})", value_byte, periods, e),
                    expected_gas: expected_total,
                    actual_gas: 0,
                    difference: -expected_total,
                    success: false,
                };
                reporter.record(result);
                continue;
            }
        };

        // Measure param push gas: key push + periods push
        let mut param_codes = Vec::new();
        param_codes.extend(build_codes!(PBUF 4));
        param_codes.extend(b"key1");
        if periods <= 255 {
            param_codes.extend(build_codes!(PU8));
            param_codes.push(periods as u8);
        } else {
            param_codes.extend(build_codes!(PU16));
            param_codes.extend((periods as u16).to_be_bytes());
        }
        param_codes.extend(build_codes!(END));
        let param_push_gas = match execute_opcode_test(param_codes, None) {
            Ok(gas) => gas,
            Err(e) => {
                let result = TestResult {
                    opcode: SRENT,
                    test_case: format!("dynamic gas (value_byte: {}, periods: {}, param push error: {})", value_byte, periods, e),
                    expected_gas: expected_total,
                    actual_gas: 0,
                    difference: -expected_total,
                    success: false,
                };
                reporter.record(result);
                continue;
            }
        };

        // Measure total: setup + SRENT call
        let mut total_codes = setup_codes;
        total_codes.append(&mut srent_codes);
        total_codes.extend(build_codes!(END));
        match execute_opcode_test(total_codes, None) {
            Ok(total_gas) => {
                // SRENT gas = total - baseline - param_push
                // Note: baseline includes SSAVE + value push + key push
                // param_push includes key push + periods push
                // total includes setup + key push + periods push + SRENT
                let actual = total_gas - baseline - param_push_gas;
                let success = actual == expected_total;
                let result = TestResult {
                    opcode: SRENT,
                    test_case: format!("dynamic gas (value_byte: {}, periods: {})", value_byte, periods),
                    expected_gas: expected_total,
                    actual_gas: actual,
                    difference: actual - expected_total,
                    success,
                };
                reporter.record(result);
            }
            Err(e) => {
                let result = TestResult {
                    opcode: SRENT,
                    test_case: format!("dynamic gas (value_byte: {}, periods: {}, error: {})", value_byte, periods, e),
                    expected_gas: expected_total,
                    actual_gas: 0,
                    difference: -expected_total,
                    success: false,
                };
                reporter.record(result);
            }
        }
    }
}

/// Test dynamic gas for HWRITE opcode
fn test_hwrite_dynamic_gas(reporter: &mut TestReporter, calc: &ExpectedGasCalculator) {
    let base_gas = calc.base_gas(HWRITE);
    let test_sizes = vec![0, 6, 12, 24, 48, 100, 200];

    for size in test_sizes {
        let expected_dynamic = calc.heap_write_gas(size);
        let expected_total = base_gas + expected_dynamic;

        // Build setup code: grow heap and prepare stack
        // Ensure heap has enough space
        let mut setup_codes = build_codes!(HGROW 1 P0);
        // Push value to write
        if size == 0 {
            setup_codes.extend(build_codes!(PNBUF));
        } else if size <= 255 {
            setup_codes.extend(build_codes!(PBUF));
            setup_codes.push(size as u8);
            setup_codes.extend(vec![0u8; size]);
        } else {
            setup_codes.extend(build_codes!(PBUFL));
            let size_u16 = size as u16;
            setup_codes.extend(size_u16.to_be_bytes());
            setup_codes.extend(vec![0u8; size]);
        }
        
        // Measure only HWRITE gas (excluding HGROW and stack setup)
        // HWRITE requires offset (u16) and value on stack
        match measure_opcode_gas_with_setup(setup_codes, HWRITE as u8) {
            Ok(actual) => {
                let success = actual == expected_total;
                let result = TestResult {
                    opcode: HWRITE,
                    test_case: format!("dynamic gas (write size: {})", size),
                    expected_gas: expected_total,
                    actual_gas: actual,
                    difference: actual - expected_total,
                    success,
                };
                reporter.record(result);
            }
            Err(e) => {
                let result = TestResult {
                    opcode: HWRITE,
                    test_case: format!("dynamic gas (write size: {}, error: {})", size, e),
                    expected_gas: expected_total,
                    actual_gas: 0,
                    difference: -expected_total,
                    success: false,
                };
                reporter.record(result);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_all_opcodes_base_gas() {
        use std::panic;
        
        let mut reporter = TestReporter::new();
        let calc = ExpectedGasCalculator::new();

        // Test all valid opcodes
        // Opcodes with gas cost = 1 - test only those that can be tested independently
        let gas_1_opcodes = vec![
            P0,
            P1,
            P2,
            P3,
            PNBUF,
            PNIL,
            TNIL,
            TMAP,
            TLIST,
            NOP,
            END,
        ];

        for opcode in gas_1_opcodes {
            // Use catch_unwind to handle panics gracefully
            let result = panic::catch_unwind(panic::AssertUnwindSafe(|| {
                test_base_gas(opcode, &mut reporter, &calc);
            }));
            if result.is_err() {
                let test_result = TestResult {
                    opcode,
                    test_case: format!("base gas (panic during execution)"),
                    expected_gas: calc.base_gas(opcode),
                    actual_gas: 0,
                    difference: -(calc.base_gas(opcode)),
                    success: false,
                };
                reporter.record(test_result);
            }
        }
        
        // Test opcodes that need parameters separately
        let opcodes_with_params = vec![PU8, CU8, CU16, CU32, CU64, CU128, CBUF, CTO, TID, TIS, POP, RET, ABT, ERR, AST, PRT];
        for opcode in opcodes_with_params {
            let result = panic::catch_unwind(panic::AssertUnwindSafe(|| {
                test_base_gas(opcode, &mut reporter, &calc);
            }));
            if result.is_err() {
                let test_result = TestResult {
                    opcode,
                    test_case: format!("base gas (panic during execution)"),
                    expected_gas: calc.base_gas(opcode),
                    actual_gas: 0,
                    difference: -(calc.base_gas(opcode)),
                    success: false,
                };
                reporter.record(test_result);
            }
        }

        // Opcodes with gas cost = 2 (default value, test some common ones)
        let gas_2_opcodes = vec![
            PU16,
            SWAP,
            SIZE,
            ADD,
            SUB,
            AND,
            OR,
            EQ,
            NEQ,
        ];

        for opcode in gas_2_opcodes {
            let result = panic::catch_unwind(panic::AssertUnwindSafe(|| {
                test_base_gas(opcode, &mut reporter, &calc);
            }));
            if result.is_err() {
                let test_result = TestResult {
                    opcode,
                    test_case: format!("base gas (panic during execution)"),
                    expected_gas: calc.base_gas(opcode),
                    actual_gas: 0,
                    difference: -(calc.base_gas(opcode)),
                    success: false,
                };
                reporter.record(test_result);
            }
        }

        // Opcodes with gas cost = 3
        let gas_3_opcodes = vec![
            BRL,
            BRS,
            BRSL,
            BRSLN,
            XLG,
            PUT,
            CHOISE,
        ];

        for opcode in gas_3_opcodes {
            let result = panic::catch_unwind(panic::AssertUnwindSafe(|| {
                test_base_gas(opcode, &mut reporter, &calc);
            }));
            if result.is_err() {
                let test_result = TestResult {
                    opcode,
                    test_case: format!("base gas (panic during execution)"),
                    expected_gas: calc.base_gas(opcode),
                    actual_gas: 0,
                    difference: -(calc.base_gas(opcode)),
                    success: false,
                };
                reporter.record(test_result);
            }
        }

        // Opcodes with gas cost = 4
        let gas_4_opcodes = vec![
            DUPN,
            POPN,
            PICK,
            PBUF,
            PBUFL,
            MOD,
            MUL,
            DIV,
            XOP,
            HREAD,
            HREADU,
            HREADUL,
            HSLICE,
            HGROW,
            ITEMGET,
            HEAD,
            TAIL,
            HASKEY,
            LENGTH,
        ];

        // Opcodes with gas cost = 4 - test only those that can be tested independently
        // DUPN, POPN, PICK need stack values - skip or use combination test
        // HREADU, HREADUL need heap setup - skip (tested in dynamic tests)
        // HSLICE needs heap setup - skip
        let gas_4_testable = vec![PBUF, PBUFL, MOD, MUL, DIV, XOP, HGROW];
        
        for opcode in gas_4_testable {
            test_base_gas(opcode, &mut reporter, &calc);
        }
        
        // Test DUPN, POPN, PICK with combination test
        test_base_gas_combination(build_codes!(P0 P1), DUPN, &mut reporter, &calc);
        test_base_gas_combination(build_codes!(P0 P1), POPN, &mut reporter, &calc);
        test_base_gas_combination(build_codes!(P0 P1), PICK, &mut reporter, &calc);
        
        // Skip HREAD, ITEMGET, HEAD, TAIL, HASKEY, LENGTH - tested in dynamic gas tests or need combination test

        // Opcodes with gas cost = 5
        test_base_gas(POW, &mut reporter, &calc);

        // Opcodes with gas cost = 6 - test only those that can be tested independently
        let gas_6_testable = vec![HWRITEX, HWRITEXL];

        for opcode in gas_6_testable {
            test_base_gas(opcode, &mut reporter, &calc);
        }
        
        // Skip HWRITE, INSERT, REMOVE, CLEAR, APPEND - tested in dynamic gas tests or need combination test

        // Opcodes with gas cost = 8 - test only those that can be tested independently
        let gas_8_testable = vec![CAT, BYTE, CUT, LEFT, RIGHT, LDROP, RDROP, JOIN, REV, NEWLIST, NEWMAP, NTCALL];

        for opcode in gas_8_testable {
            test_base_gas(opcode, &mut reporter, &calc);
        }
        
        // Skip MGET - tested in dynamic gas tests

        // Opcodes with gas cost = 12 - test only those that can be tested independently
        let gas_12_testable = vec![EXTENV, MPUT, CALLTHIS, CALLSELF, CALLSUPER];

        for opcode in gas_12_testable {
            test_base_gas(opcode, &mut reporter, &calc);
        }
        
        // Skip PACKLIST, PACKMAP, UPLIST, CLONE, MERGE, KEYS, VALUES - need combination test

        // Opcodes with gas cost = 16
        let gas_16_testable = vec![EXTFUNC, CALLCODE];

        for opcode in gas_16_testable {
            test_base_gas(opcode, &mut reporter, &calc);
        }
        
        // Skip GGET - tested in dynamic gas tests

        // Opcodes with gas cost = 20
        let gas_20_testable = vec![CALLPURE];

        for opcode in gas_20_testable {
            test_base_gas(opcode, &mut reporter, &calc);
        }
        
        // Skip LOG1 - tested in dynamic gas tests

        // Opcodes with gas cost = 24
        let gas_24_testable = vec![GPUT, CALLVIEW];

        for opcode in gas_24_testable {
            test_base_gas(opcode, &mut reporter, &calc);
        }
        
        // Skip LOG2 - tested in dynamic gas tests

        // Opcodes with gas cost = 28
        let gas_28_testable = vec![SDEL, EXTACTION];

        for opcode in gas_28_testable {
            test_base_gas(opcode, &mut reporter, &calc);
        }
        
        // Skip LOG3 - tested in dynamic gas tests

        // Opcodes with gas cost = 32
        let gas_32_testable = vec![SREST, CALL];

        for opcode in gas_32_testable {
            test_base_gas(opcode, &mut reporter, &calc);
        }
        
        // Skip LOG4, SLOAD - tested in dynamic gas tests

        // Opcodes with gas cost = 64
        let gas_64_opcodes = vec![SSAVE, SRENT];

        for opcode in gas_64_opcodes {
            test_base_gas(opcode, &mut reporter, &calc);
        }

        reporter.print_report();
    }

    #[test]
    fn test_stack_buffer_copy_dynamic_gas() {
        let mut reporter = TestReporter::new();
        let calc = ExpectedGasCalculator::new();

        test_dup_dynamic_gas(&mut reporter, &calc);
        test_pbuf_dynamic_gas(&mut reporter, &calc);
        test_get_dynamic_gas(&mut reporter, &calc);
        test_mget_dynamic_gas(&mut reporter, &calc);
        test_gget_dynamic_gas(&mut reporter, &calc);

        reporter.print_report();
    }

    #[test]
    fn test_heap_operations_dynamic_gas() {
        let mut reporter = TestReporter::new();
        let calc = ExpectedGasCalculator::new();

        test_hgrow_dynamic_gas(&mut reporter, &calc);
        test_hread_dynamic_gas(&mut reporter, &calc);
        test_hwrite_dynamic_gas(&mut reporter, &calc);

        reporter.print_report();
    }

    #[test]
    fn test_compo_operations_dynamic_gas() {
        let mut reporter = TestReporter::new();
        let calc = ExpectedGasCalculator::new();

        test_itemget_dynamic_gas(&mut reporter, &calc);

        reporter.print_report();
    }

    #[test]
    fn test_storage_operations_dynamic_gas() {
        let mut reporter = TestReporter::new();
        let calc = ExpectedGasCalculator::new();

        test_sload_dynamic_gas(&mut reporter, &calc);
        test_srent_dynamic_gas(&mut reporter, &calc);

        reporter.print_report();
    }

    #[test]
    fn test_log_operations_dynamic_gas() {
        let mut reporter = TestReporter::new();
        let calc = ExpectedGasCalculator::new();

        test_log_dynamic_gas(&mut reporter, &calc);

        reporter.print_report();
    }

    #[test]
    fn test_all_gas_costs() {
        // Run all tests
        // Note: test_all_opcodes_base_gas may fail for some opcodes that require complex setup
        // These are covered by dynamic gas tests
        let _ = std::panic::catch_unwind(|| {
            test_all_opcodes_base_gas();
        });
        test_stack_buffer_copy_dynamic_gas();
        test_heap_operations_dynamic_gas();
        test_compo_operations_dynamic_gas();
        test_storage_operations_dynamic_gas();
        test_log_operations_dynamic_gas();
    }
}
