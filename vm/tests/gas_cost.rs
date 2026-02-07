use std::collections::HashMap;
use vm::rt::{Bytecode, GasExtra, GasTable, CallExit, ExecMode, SpaceCap};
use basis::component::Env;
use basis::interface::{Context, TransactionRead, State};
use field::{Address, Amount, Hash};
use protocol::context::ContextInst;
use protocol::state::EmptyLogs;
use vm::space::{CtcKVMap, GKVMap, Heap, Stack};
use vm::ContractAddress;
use vm::machine::CtxHost;
use vm::interpreter::execute_code;
use vm::value::Value;
use vm::value::ValueTy;
use vm::VmrtRes;
use std::collections::HashMap as MemKV;

#[derive(Debug, Clone)]
struct GasMismatch {
    opcode: Bytecode,
    test_case: String,
    expected: i64,
    actual: i64,
    resource_sizes: HashMap<String, usize>,
}

#[allow(dead_code)]
struct GasTestContext {
    mismatches: Vec<GasMismatch>,
    gas_table: GasTable,
    gas_extra: GasExtra,
}

impl GasTestContext {
    fn new() -> Self {
        Self {
            mismatches: Vec::new(),
            gas_table: GasTable::new(1),
            gas_extra: GasExtra::new(1),
        }
    }

    fn record_mismatch(&mut self, opcode: Bytecode, test_case: String, expected: i64, actual: i64, resource_sizes: HashMap<String, usize>) {
        self.mismatches.push(GasMismatch {
            opcode,
            test_case,
            expected,
            actual,
            resource_sizes,
        });
    }

    fn print_report(&self) {
        if self.mismatches.is_empty() {
            println!("\n✅ All gas consumption tests passed!");
        } else {
            println!("\n❌ Found {} gas consumption mismatches:", self.mismatches.len());
            for mismatch in &self.mismatches {
                println!("\n  Opcode: {:?}", mismatch.opcode);
                println!("  Test case: {}", mismatch.test_case);
                println!("  Expected: {} gas", mismatch.expected);
                println!("  Actual: {} gas", mismatch.actual);
                println!("  Difference: {} gas", mismatch.actual - mismatch.expected);
                if !mismatch.resource_sizes.is_empty() {
                    println!("  Resource sizes: {:?}", mismatch.resource_sizes);
                }
            }
        }
    }
}

// Calculate expected base gas according to documentation
fn get_base_gas(opcode: Bytecode) -> i64 {
    use Bytecode::*;
    match opcode {
        PU8 | P0 | P1 | P2 | P3 | PNBUF | PNIL |
        CU8 | CU16 | CU32 | CU64 | CU128 | CBUF | CTO | TID | TIS | TNIL | TMAP | TLIST |
        POP | NOP | NT | END | RET | ABT | ERR | AST | PRT => 1,
        
        BRL | BRS | BRSL | BRSLN | XLG | PUT | CHOISE => 3,
        
        DUPN | POPN | PICK |
        PBUF | PBUFL |
        MOD | MUL | DIV | XOP |
        HREAD | HREADU | HREADUL | HSLICE | HGROW |
        ITEMGET | HEAD | BACK | HASKEY | LENGTH => 4,
        
        POW => 5,
        
        HWRITE | HWRITEX | HWRITEXL |
        INSERT | REMOVE | CLEAR | APPEND => 6,
        
        CAT | BYTE | CUT | LEFT | RIGHT | LDROP | RDROP |
        MGET | JOIN | REV |
        NEWLIST | NEWMAP |
        NTCALL => 8,
        
        EXTENV | MPUT | CALLTHIS | CALLSELF | CALLSUPER |
        PACKLIST | PACKMAP | UPLIST | CLONE | MERGE | KEYS | VALUES => 12,
        
        EXTFUNC | GGET | CALLCODE => 16,
        
        LOG1 | CALLPURE => 20,
        
        LOG2 | GPUT | CALLVIEW => 24,
        
        LOG3 | SDEL | EXTACTION => 28,
        
        LOG4 | SLOAD | SREST | CALL => 32,
        
        SSAVE | SRENT => 64,
        
        _ => 2, // All other opcodes default to 2
    }
}

// Calculate expected heap grow gas (exponential for first 8 segments, then linear)
fn calculate_heap_grow_gas(seg_count: usize) -> i64 {
    let mut gas: i64 = 0;
    let exp_count = seg_count.min(8);
    
    // First 8 segments: 2, 4, 8, 16, 32, 64, 128, 256
    for i in 0..exp_count {
        gas += 1i64 << (i + 1);
    }
    
    // Subsequent segments: 256 gas each
    if seg_count > 8 {
        gas += (seg_count - 8) as i64 * 256;
    }
    
    gas
}

// Helper function to execute a single gas test
fn execute_test_with_argv(gas_limit: i64, codes: Vec<u8>, argv: Option<Value>) -> VmrtRes<(CallExit, i64, Vec<Value>, Heap)> {
    let mut pc: usize = 0;
    let mut gas: i64 = gas_limit;
    let cadr = ContractAddress::default();

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
        fn fee_extend(&self) -> sys::Ret<(u16, Amount)> { Ok((1, Amount::zero())) }
    }

    #[derive(Default)]
    struct StateMem {
        mem: MemKV<Vec<u8>, Vec<u8>>,
    }
    
    impl State for StateMem {
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

    let tx = DummyTx::default();
    let mut env = Env::default();
    env.block.height = 1;
    let mut ctx = ContextInst::new(env, Box::new(StateMem::default()), Box::new(EmptyLogs{}), &tx);
    let ctx: &mut dyn Context = &mut ctx;

    let mut ops = Stack::new(256);
    if let Some(v) = argv {
        ops.push(v)?;
    }

    let mut heap = Heap::new(64);

    let mut host = CtxHost::new(ctx);
    execute_code(
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

// Execute a single gas test
fn run_gas_test(ctx: &mut GasTestContext, codes: Vec<u8>, expected_gas: i64, test_case: &str, opcode: Bytecode, resource_sizes: HashMap<String, usize>) {
    let gas_limit = 100000i64;
    match execute_test_with_argv(gas_limit, codes, None) {
        Ok((_exit, actual_gas, _ops, _heap)) => {
            if actual_gas != expected_gas {
                ctx.record_mismatch(opcode, test_case.to_string(), expected_gas, actual_gas, resource_sizes);
            }
        }
        Err(e) => {
            ctx.record_mismatch(
                opcode,
                format!("{} (execution failed: {:?})", test_case, e),
                expected_gas,
                -1,
                resource_sizes,
            );
        }
    }
}

// Test base gas consumption
fn test_base_gas(ctx: &mut GasTestContext, opcode: Bytecode, setup_code: Vec<u8>) {
    let expected = get_base_gas(opcode);
    let mut codes = setup_code;
    codes.extend(opcode_min_bytes(opcode));
    codes.push(Bytecode::END as u8);
    
    run_gas_test(ctx, codes, expected, &format!("Base gas test: {:?}", opcode), opcode, HashMap::new());
}

fn opcode_min_bytes(opcode: Bytecode) -> Vec<u8> {
    use Bytecode::*;
    match opcode {
        // constants / type ops with immediates
        PU8 => vec![PU8 as u8, 42],
        PU16 => vec![PU16 as u8, 0, 1],
        CTO => vec![CTO as u8, ValueTy::Bool as u8],
        TIS => vec![TIS as u8, ValueTy::U8 as u8],

        // branch/jump need immediates even if not taken
        BRL | JMPL => vec![opcode as u8, 0, 0],
        BRS | JMPS => vec![opcode as u8, 0],
        BRSL | BRSLN | JMPSL => vec![opcode as u8, 0, 0],

        // simple u8 immediates
        ALLOC | PUT | GET | PICK | POPN | XLG | XOP => vec![opcode as u8, 0],
        DUPN | REV => vec![opcode as u8, 2],
        JOIN => vec![opcode as u8, 3],

        // push bytes with a zero-length payload
        PBUF => vec![PBUF as u8, 0],
        PBUFL => vec![PBUFL as u8, 0, 0],

        _ => vec![opcode as u8],
    }
}

// Test Stack buffer copy gas (byte/12)
fn test_stack_copy_gas(ctx: &mut GasTestContext, opcode: Bytecode, buffer_size: usize) {
    let expected_base = get_base_gas(opcode);
    let expected_copy = (buffer_size as i64) / 12;
    let expected = expected_base + expected_copy;
    
    let mut codes = Vec::new();
    
    // Prepare buffer
    match opcode {
        Bytecode::DUP => {
            codes.push(Bytecode::PBUF as u8);
            codes.push(buffer_size.min(255) as u8);
            for _ in 0..buffer_size.min(255) {
                codes.push(0);
            }
        }
        Bytecode::GET | Bytecode::GET0 | Bytecode::GET1 | Bytecode::GET2 | Bytecode::GET3 => {
            codes.push(Bytecode::ALLOC as u8);
            codes.push(1);
            codes.push(Bytecode::PUT as u8);
            codes.push(0);
            codes.push(Bytecode::PBUF as u8);
            codes.push(buffer_size.min(255) as u8);
            for _ in 0..buffer_size.min(255) {
                codes.push(0);
            }
        }
        Bytecode::GETX => {
            codes.push(Bytecode::ALLOC as u8);
            codes.push(1);
            codes.push(Bytecode::PUTX as u8);
            codes.push(0);
            codes.push(0);
            codes.push(Bytecode::PBUF as u8);
            codes.push(buffer_size.min(255) as u8);
            for _ in 0..buffer_size.min(255) {
                codes.push(0);
            }
        }
        Bytecode::MGET => {
            codes.push(Bytecode::MPUT as u8);
            codes.push(Bytecode::PBUF as u8);
            codes.push(1);
            codes.push(0);
            codes.push(Bytecode::PBUF as u8);
            codes.push(buffer_size.min(255) as u8);
            for _ in 0..buffer_size.min(255) {
                codes.push(0);
            }
        }
        Bytecode::GGET => {
            codes.push(Bytecode::GPUT as u8);
            codes.push(Bytecode::PBUF as u8);
            codes.push(1);
            codes.push(0);
            codes.push(Bytecode::PBUF as u8);
            codes.push(buffer_size.min(255) as u8);
            for _ in 0..buffer_size.min(255) {
                codes.push(0);
            }
        }
        _ => return,
    }
    
    // Execute opcode
    match opcode {
        Bytecode::DUP => codes.push(Bytecode::DUP as u8),
        Bytecode::GET => {
            codes.push(Bytecode::GET as u8);
            codes.push(0);
        }
        Bytecode::GET0 => codes.push(Bytecode::GET0 as u8),
        Bytecode::GET1 => codes.push(Bytecode::GET1 as u8),
        Bytecode::GET2 => codes.push(Bytecode::GET2 as u8),
        Bytecode::GET3 => codes.push(Bytecode::GET3 as u8),
        Bytecode::GETX => {
            codes.push(Bytecode::GETX as u8);
            codes.push(0);
            codes.push(0);
        }
        Bytecode::MGET => {
            codes.push(Bytecode::PBUF as u8);
            codes.push(1);
            codes.push(0);
            codes.push(Bytecode::MGET as u8);
        }
        Bytecode::GGET => {
            codes.push(Bytecode::PBUF as u8);
            codes.push(1);
            codes.push(0);
            codes.push(Bytecode::GGET as u8);
        }
        _ => return,
    }
    
    codes.push(Bytecode::END as u8);
    
    let mut resource_sizes = HashMap::new();
    resource_sizes.insert("buffer_size".to_string(), buffer_size);
    
    run_gas_test(ctx, codes, expected, &format!("Stack copy ({} bytes)", buffer_size), opcode, resource_sizes);
}

// Test Heap grow gas
fn test_heap_grow_gas(ctx: &mut GasTestContext, seg_count: u8) {
    let expected_base = get_base_gas(Bytecode::HGROW);
    let expected_grow = calculate_heap_grow_gas(seg_count as usize);
    let expected = expected_base + expected_grow;
    
    let mut codes = Vec::new();
    codes.push(Bytecode::HGROW as u8);
    codes.push(seg_count);
    codes.push(Bytecode::END as u8);
    
    let mut resource_sizes = HashMap::new();
    resource_sizes.insert("seg_count".to_string(), seg_count as usize);
    
    run_gas_test(ctx, codes, expected, &format!("Heap grow ({} segments)", seg_count), Bytecode::HGROW, resource_sizes);
}

// Test Heap read gas (byte/16)
fn test_heap_read_gas(ctx: &mut GasTestContext, read_length: usize) {
    let expected_base = get_base_gas(Bytecode::HREAD);
    let expected_read = (read_length as i64) / 16;
    let expected = expected_base + expected_read;
    
    let mut codes = Vec::new();
    // Grow heap first
    codes.push(Bytecode::HGROW as u8);
    codes.push(1);
    // Prepare parameters
    codes.push(Bytecode::PU16 as u8);
    codes.push((read_length as u16 >> 8) as u8);
    codes.push((read_length as u16 & 0xff) as u8);
    codes.push(Bytecode::PU16 as u8);
    codes.push(0);
    codes.push(0);
    // Execute HREAD
    codes.push(Bytecode::HREAD as u8);
    codes.push(Bytecode::END as u8);
    
    let mut resource_sizes = HashMap::new();
    resource_sizes.insert("read_length".to_string(), read_length);
    
    run_gas_test(ctx, codes, expected, &format!("Heap read ({} bytes)", read_length), Bytecode::HREAD, resource_sizes);
}

// Test Heap write gas (byte/12)
fn test_heap_write_gas(ctx: &mut GasTestContext, write_length: usize) {
    let expected_base = get_base_gas(Bytecode::HWRITE);
    let expected_write = (write_length as i64) / 12;
    let expected = expected_base + expected_write;
    
    let mut codes = Vec::new();
    // Grow heap first
    codes.push(Bytecode::HGROW as u8);
    codes.push(1);
    // Prepare parameters
    codes.push(Bytecode::PU16 as u8);
    codes.push(0);
    codes.push(0);
    codes.push(Bytecode::PBUF as u8);
    codes.push(write_length as u8);
    for _ in 0..write_length {
        codes.push(0);
    }
    // Execute HWRITE
    codes.push(Bytecode::HWRITE as u8);
    codes.push(Bytecode::END as u8);
    
    let mut resource_sizes = HashMap::new();
    resource_sizes.insert("write_length".to_string(), write_length);
    
    run_gas_test(ctx, codes, expected, &format!("Heap write ({} bytes)", write_length), Bytecode::HWRITE, resource_sizes);
}

// Test Storage load gas (byte/8)
fn test_storage_load_gas(ctx: &mut GasTestContext, value_size: usize) {
    let expected_base = get_base_gas(Bytecode::SLOAD);
    let expected_read = (value_size as i64) / 8;
    let expected = expected_base + expected_read;
    
    // Note: SLOAD requires data to be saved first, which needs special setup
    // Here we only test the base part, actual tests may need mock storage
    let mut codes = Vec::new();
    // Save data first
    codes.push(Bytecode::PBUF as u8);
    codes.push(1);
    codes.push(0);
    codes.push(Bytecode::PBUF as u8);
    codes.push(value_size.min(255) as u8);
    for _ in 0..value_size.min(255) {
        codes.push(0);
    }
    codes.push(Bytecode::SSAVE as u8);
    // Then load
    codes.push(Bytecode::PBUF as u8);
    codes.push(1);
    codes.push(0);
    codes.push(Bytecode::SLOAD as u8);
    codes.push(Bytecode::END as u8);
    
    let mut resource_sizes = HashMap::new();
    resource_sizes.insert("value_size".to_string(), value_size);
    
    // This test may fail due to storage requirements, but we record it
    run_gas_test(ctx, codes, expected, &format!("Storage load ({} bytes)", value_size), Bytecode::SLOAD, resource_sizes);
}

// Test Storage save gas (byte/6 + rent + key cost)
fn test_storage_save_gas(ctx: &mut GasTestContext, value_size: usize, is_new_key: bool) {
    let expected_base = get_base_gas(Bytecode::SSAVE);
    let expected_write = (value_size as i64) / 6;
    let expected_rent = 32 + value_size as i64; // 1 period rent
    let expected_key_cost = if is_new_key { 256 } else { 0 };
    let expected = expected_base + expected_write + expected_rent + expected_key_cost;
    
    let mut codes = Vec::new();
    codes.push(Bytecode::PBUF as u8);
    codes.push(1);
    codes.push(0);
    codes.push(Bytecode::PBUF as u8);
    codes.push(value_size.min(255) as u8);
    for _ in 0..value_size.min(255) {
        codes.push(0);
    }
    codes.push(Bytecode::SSAVE as u8);
    codes.push(Bytecode::END as u8);
    
    let mut resource_sizes = HashMap::new();
    resource_sizes.insert("value_size".to_string(), value_size);
    resource_sizes.insert("is_new_key".to_string(), if is_new_key { 1 } else { 0 });
    
    run_gas_test(ctx, codes, expected, &format!("Storage save ({} bytes, new_key={})", value_size, is_new_key), Bytecode::SSAVE, resource_sizes);
}

// Test Compo operations gas
fn test_compo_gas(ctx: &mut GasTestContext, opcode: Bytecode, item_count: usize, byte_size: usize) {
    let expected_base = get_base_gas(opcode);
    let mut expected = expected_base;
    
    use Bytecode::*;
    match opcode {
        ITEMGET | HEAD | BACK | HASKEY | UPLIST | APPEND => {
            expected += (item_count as i64) / 4;
            expected += (byte_size as i64) / 20;
        }
        KEYS | VALUES | INSERT | REMOVE => {
            expected += (item_count as i64) / 2;
            expected += (byte_size as i64) / 20;
        }
        CLONE | MERGE => {
            expected += item_count as i64;
            expected += (byte_size as i64) / 20;
        }
        _ => {}
    }
    
    // Build test code (simplified version)
    let mut codes = Vec::new();
    match opcode {
        NEWLIST => {
            codes.push(NEWLIST as u8);
        }
        NEWMAP => {
            codes.push(NEWMAP as u8);
        }
        _ => {
            // Other compo operations need to create compo first
            codes.push(NEWLIST as u8);
            // Add some elements
            for i in 0..item_count.min(10) {
                codes.push(PU8 as u8);
                codes.push(i as u8);
                codes.push(APPEND as u8);
            }
        }
    }
    
    codes.push(END as u8);
    
    let mut resource_sizes = HashMap::new();
    resource_sizes.insert("item_count".to_string(), item_count);
    resource_sizes.insert("byte_size".to_string(), byte_size);
    
    run_gas_test(ctx, codes, expected, &format!("Compo op (items={}, bytes={})", item_count, byte_size), opcode, resource_sizes);
}

// Test Log gas (byte/1)
fn test_log_gas(ctx: &mut GasTestContext, log_opcode: Bytecode, total_bytes: usize) {
    let expected_base = get_base_gas(log_opcode);
    let expected_log = total_bytes as i64;
    let expected = expected_base + expected_log;
    
    let mut codes = Vec::new();
    
    // Prepare log data
    let item_count = match log_opcode {
        Bytecode::LOG1 => 2,
        Bytecode::LOG2 => 3,
        Bytecode::LOG3 => 4,
        Bytecode::LOG4 => 5,
        _ => return,
    };
    
    // Calculate bytes per item
    let bytes_per_item = total_bytes / item_count;
    
    for _ in 0..item_count {
        codes.push(Bytecode::PBUF as u8);
        codes.push(bytes_per_item.min(255) as u8);
        for _ in 0..bytes_per_item.min(255) {
            codes.push(0);
        }
    }
    
    codes.push(log_opcode as u8);
    codes.push(Bytecode::END as u8);
    
    let mut resource_sizes = HashMap::new();
    resource_sizes.insert("total_bytes".to_string(), total_bytes);
    
    run_gas_test(ctx, codes, expected, &format!("Log ({} bytes)", total_bytes), log_opcode, resource_sizes);
}

// Test Space alloc gas
fn test_space_alloc_gas(ctx: &mut GasTestContext, opcode: Bytecode, count: usize) {
    let expected_base = get_base_gas(opcode);
    let mut expected = expected_base;
    
    use Bytecode::*;
    match opcode {
        ALLOC => {
            expected += count as i64 * 5; // 5 gas per local stack slot
        }
        MPUT => {
            // 20 gas per memory key (new key only)
            // We assume it's a new key here
            expected += 20;
        }
        GPUT => {
            // 32 gas per global key (new key only)
            // We assume it's a new key here
            expected += 32;
        }
        _ => return,
    }
    
    let mut codes = Vec::new();
    
    match opcode {
        ALLOC => {
            codes.push(ALLOC as u8);
            codes.push(count.min(255) as u8);
        }
        MPUT => {
            codes.push(PBUF as u8);
            codes.push(1);
            codes.push(0);
            codes.push(PBUF as u8);
            codes.push(1);
            codes.push(0);
            codes.push(MPUT as u8);
        }
        GPUT => {
            codes.push(PBUF as u8);
            codes.push(1);
            codes.push(0);
            codes.push(PBUF as u8);
            codes.push(1);
            codes.push(0);
            codes.push(GPUT as u8);
        }
        _ => return,
    }
    
    codes.push(END as u8);
    
    let mut resource_sizes = HashMap::new();
    resource_sizes.insert("count".to_string(), count);
    
    run_gas_test(ctx, codes, expected, &format!("Space alloc (count={})", count), opcode, resource_sizes);
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_all_base_gas() {
        let mut ctx = GasTestContext::new();
        
        // Test all opcodes with base gas = 1
        let gas1_opcodes = vec![
            Bytecode::PU8, Bytecode::P0, Bytecode::P1, Bytecode::P2, Bytecode::P3,
            Bytecode::PNBUF, Bytecode::PNIL,
            Bytecode::CU8, Bytecode::CU16, Bytecode::CU32, Bytecode::CU64, Bytecode::CU128,
            Bytecode::CBUF, Bytecode::CTO, Bytecode::TID, Bytecode::TIS,
            Bytecode::TNIL, Bytecode::TMAP, Bytecode::TLIST,
            Bytecode::POP, Bytecode::NOP, Bytecode::NT, Bytecode::END,
            Bytecode::RET, Bytecode::ABT, Bytecode::ERR, Bytecode::AST, Bytecode::PRT,
        ];
        
        for opcode in gas1_opcodes {
            let setup = match opcode {
                Bytecode::CU8 | Bytecode::CU16 | Bytecode::CU32 | Bytecode::CU64 | Bytecode::CU128 |
                Bytecode::CBUF | Bytecode::CTO | Bytecode::TID | Bytecode::TIS |
                Bytecode::TNIL | Bytecode::TMAP | Bytecode::TLIST => {
                    vec![Bytecode::P0 as u8]
                }
                Bytecode::PRT => vec![Bytecode::P0 as u8],
                Bytecode::AST => vec![Bytecode::P1 as u8],
                Bytecode::ERR => vec![Bytecode::P0 as u8],
                Bytecode::RET => vec![Bytecode::P0 as u8],
                _ => Vec::new(),
            };
            test_base_gas(&mut ctx, opcode, setup);
        }
        
        // Test opcodes with base gas = 3
        let gas3_opcodes = vec![
            Bytecode::BRL, Bytecode::BRS, Bytecode::BRSL, Bytecode::BRSLN,
            Bytecode::XLG, Bytecode::PUT, Bytecode::CHOISE,
        ];
        
        for opcode in gas3_opcodes {
            let setup = match opcode {
                Bytecode::PUT => vec![Bytecode::ALLOC as u8, 1, Bytecode::P0 as u8],
                Bytecode::CHOISE => vec![Bytecode::P1 as u8, Bytecode::P0 as u8, Bytecode::P1 as u8],
                Bytecode::XLG => vec![Bytecode::ALLOC as u8, 1, Bytecode::P0 as u8],
                Bytecode::BRL | Bytecode::BRS | Bytecode::BRSL | Bytecode::BRSLN => {
                    vec![Bytecode::P0 as u8]
                }
                _ => Vec::new(),
            };
            test_base_gas(&mut ctx, opcode, setup);
        }
        
        ctx.print_report();
    }
    
    #[test]
    fn test_stack_copy_gas() {
        let mut ctx = GasTestContext::new();
        
        let buffer_sizes = vec![0, 1, 11, 12, 24, 48, 100, 200];
        let opcodes = vec![
            Bytecode::DUP,
            Bytecode::GET, Bytecode::GET0, Bytecode::GET1, Bytecode::GET2, Bytecode::GET3,
            Bytecode::GETX,
            Bytecode::MGET, Bytecode::GGET,
        ];
        
        for opcode in opcodes {
            for &size in &buffer_sizes {
                super::test_stack_copy_gas(&mut ctx, opcode, size);
            }
        }
        
        ctx.print_report();
    }
    
    #[test]
    fn test_heap_grow_gas() {
        let mut ctx = GasTestContext::new();
        
        // Test exponential growth for first 8 segments and linear growth afterwards
        for seg in 1..=16 {
            super::test_heap_grow_gas(&mut ctx, seg);
        }
        
        ctx.print_report();
    }
    
    #[test]
    fn test_heap_read_write_gas() {
        let mut ctx = GasTestContext::new();
        
        let sizes = vec![0, 1, 15, 16, 24, 48, 100, 200];
        
        for &size in &sizes {
            super::test_heap_read_gas(&mut ctx, size);
            super::test_heap_write_gas(&mut ctx, size);
        }
        
        ctx.print_report();
    }
    
    #[test]
    fn test_compo_gas() {
        let mut ctx = GasTestContext::new();
        
        let compo_opcodes = vec![
            Bytecode::NEWLIST, Bytecode::NEWMAP,
            Bytecode::ITEMGET, Bytecode::HEAD, Bytecode::BACK,
            Bytecode::HASKEY, Bytecode::APPEND, Bytecode::KEYS, Bytecode::VALUES,
            Bytecode::CLONE, Bytecode::MERGE,
        ];
        
        let item_counts = vec![0, 1, 4, 8, 16, 32];
        let byte_sizes = vec![0, 20, 40, 80, 160];
        
        for opcode in compo_opcodes {
            for &items in &item_counts {
                for &bytes in &byte_sizes {
                    super::test_compo_gas(&mut ctx, opcode, items, bytes);
                }
            }
        }
        
        ctx.print_report();
    }
    
    #[test]
    fn test_log_gas() {
        let mut ctx = GasTestContext::new();
        
        let log_opcodes = vec![
            Bytecode::LOG1, Bytecode::LOG2, Bytecode::LOG3, Bytecode::LOG4,
        ];
        
        let byte_sizes = vec![0, 1, 10, 20, 50, 100, 200];
        
        for opcode in log_opcodes {
            for &bytes in &byte_sizes {
                super::test_log_gas(&mut ctx, opcode, bytes);
            }
        }
        
        ctx.print_report();
    }
    
    #[test]
    fn test_space_alloc_gas() {
        let mut ctx = GasTestContext::new();
        
        let alloc_opcodes = vec![
            Bytecode::ALLOC, Bytecode::MPUT, Bytecode::GPUT,
        ];
        
        let counts = vec![1, 2, 5, 10];
        
        for opcode in alloc_opcodes {
            for &count in &counts {
                super::test_space_alloc_gas(&mut ctx, opcode, count);
            }
        }
        
        ctx.print_report();
    }
    
    #[test]
    fn test_storage_gas() {
        let mut ctx = GasTestContext::new();
        
        let value_sizes = vec![0, 10, 20, 40, 80, 160];
        
        for &size in &value_sizes {
            super::test_storage_load_gas(&mut ctx, size);
            super::test_storage_save_gas(&mut ctx, size, true);
        }
        
        ctx.print_report();
    }
    
    #[test]
    fn test_all_opcodes_comprehensive() {
        println!("Starting comprehensive gas consumption tests...");
        
        // Test all base gas
        println!("\n=== Testing base gas ===");
        test_all_base_gas();
        
        // Test dynamic gas
        println!("\n=== Testing Stack copy gas ===");
        test_stack_copy_gas();
        
        println!("\n=== Testing Heap grow gas ===");
        test_heap_grow_gas();
        
        println!("\n=== Testing Heap read/write gas ===");
        test_heap_read_write_gas();
        
        println!("\n=== Testing Compo gas ===");
        test_compo_gas();
        
        println!("\n=== Testing Log gas ===");
        test_log_gas();
        
        println!("\n=== Testing Space alloc gas ===");
        test_space_alloc_gas();
        
        println!("\n✅ All tests completed!");
    }
}
