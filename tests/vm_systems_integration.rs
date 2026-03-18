use std::sync::Arc;

use basis::interface::ActExec;
use basis::interface::{Context, State, TxDriverContext};
use field::{Address, Amount, Field, Uint4};
use sys::{IntoTRet, Ret, XRet};
use testkit::sim::integration::{
    make_ctx_from_tx as make_ctx, make_stub_tx as make_tx, set_vm_assigner, test_guard,
    vm_main_addr as main_addr,
};
use testkit::sim::logs::MemLogs;
use testkit::sim::state::FlatMemState as StateMem;
use vm::action::ContractDeploy;
use vm::contract::{Contract, Func};
use vm::lang::lang_to_bytecode;
use vm::machine::{self, ContractCacheConfig, Machine, Resoure};
use vm::rt::{Bytecode, CodeType, FnSign, calc_func_sign};
use vm::value::Value;
use vm::{ContractAddress, ContractSto, VMState, VmLog};

fn contract_addr(main: &Address, nonce: u32) -> ContractAddress {
    ContractAddress::calculate(main, &Uint4::from(nonce))
}

fn insert_contract(state: &mut dyn State, addr: &ContractAddress, sto: &ContractSto) {
    let mut vm_state = VMState::wrap(state);
    vm_state.contract_set_sync_edition(addr, sto);
}

fn make_external_contract(func_name: &str, body: &str) -> ContractSto {
    Contract::new()
        .func(
            Func::new(func_name)
                .unwrap()
                .external()
                .fitsh(body)
                .unwrap(),
        )
        .into_sto()
}

fn execute_main_bytecode(ctx: &mut dyn TxDriverContext, codes: Vec<u8>) -> Ret<Value> {
    let main = ctx.env().tx.main;
    let _ = protocol::operate::hac_add(ctx, &main, &Amount::unit238(1_000_000_000));
    if let Some(gas_max) = ctx.tx().gas_max_byte() {
        if gas_max > 0 {
            let (budget, gas_rate) = protocol::context::tx_gas_params_from_byte(gas_max)?;
            ctx.gas_init_tx(budget, gas_rate)?;
        }
    }
    let height = ctx.env().block.height;
    let mut machine = Machine::create(Resoure::create(height));
    machine
        .main_call(ctx, CodeType::Bytecode, codes.into())
        .into_tret()
}

fn single_call_codes(lib_idx: u8, sig: FnSign) -> Vec<u8> {
    let mut codes = vec![Bytecode::PNIL as u8, Bytecode::CALLEXT as u8, lib_idx];
    codes.extend_from_slice(&sig);
    codes.push(Bytecode::END as u8);
    codes
}

fn execute_deploy(
    ctx: &mut dyn Context,
    nonce: u32,
    contract: ContractSto,
) -> XRet<(u32, Vec<u8>)> {
    let mut act = ContractDeploy::new();
    act.nonce = Uint4::from(nonce);
    act.protocol_cost = Amount::zero();
    act.contract = contract;
    act.execute(ctx)
}

#[test]
fn setup_vm_run_rejects_low_tx_type() {
    let _guard = test_guard();
    set_vm_assigner(None);

    let main = main_addr();
    let tx = make_tx(2, main, vec![], 17);
    let mut ctx = make_ctx(
        1,
        &tx,
        Box::new(StateMem::default()),
        Box::new(MemLogs::default()),
    );
    ctx.env.chain.fast_sync = true;

    let err = machine::setup_vm_run_main(
        &mut ctx,
        CodeType::Bytecode as u8,
        Arc::from(vec![Bytecode::END as u8]),
    )
    .unwrap_err();

    assert!(err.contains("current transaction type 2 too low"), "{err}");
}

#[test]
fn setup_vm_run_requires_registered_assigner() {
    let _guard = test_guard();
    set_vm_assigner(None);

    let main = main_addr();
    let tx = make_tx(3, main, vec![], 17);
    let mut ctx = make_ctx(
        1,
        &tx,
        Box::new(StateMem::default()),
        Box::new(MemLogs::default()),
    );

    let err = machine::setup_vm_run_main(
        &mut ctx,
        CodeType::Bytecode as u8,
        Arc::from(vec![Bytecode::END as u8]),
    )
    .unwrap_err();

    assert!(err.contains("vm not initialized"), "{err}");
}

#[test]
fn setup_vm_run_without_gas_init_reports_run_out() {
    let _guard = test_guard();
    set_vm_assigner(Some(|height| {
        Box::new(vm::global_machine_manager().assign(height))
    }));

    let main = main_addr();
    let tx = make_tx(3, main, vec![], 0);
    let mut ctx = make_ctx(
        1,
        &tx,
        Box::new(StateMem::default()),
        Box::new(MemLogs::default()),
    );

    let err = machine::setup_vm_run_main(
        &mut ctx,
        CodeType::Bytecode as u8,
        Arc::from(vec![Bytecode::END as u8]),
    )
    .unwrap_err();

    assert!(err.contains("gas not initialized"), "{err}");

    set_vm_assigner(None);
}

#[test]
fn setup_vm_run_executes_after_assigner_registered() {
    let _guard = test_guard();
    set_vm_assigner(Some(|height| {
        Box::new(vm::global_machine_manager().assign(height))
    }));

    let main = main_addr();
    let tx = make_tx(3, main, vec![], 17);
    let mut ctx = make_ctx(
        1,
        &tx,
        Box::new(StateMem::default()),
        Box::new(MemLogs::default()),
    );
    protocol::operate::hac_add(&mut ctx, &main, &Amount::unit238(1_000_000_000)).unwrap();
    let (budget, gas_rate) = protocol::context::tx_gas_params_from_byte(17).unwrap();
    ctx.gas_init_tx(budget, gas_rate).unwrap();

    let (gas_used, rv) = machine::setup_vm_run_main(
        &mut ctx,
        CodeType::Bytecode as u8,
        Arc::from(vec![Bytecode::END as u8]),
    )
    .unwrap();

    assert!(gas_used > 0);
    assert!(!rv.extract_bool().unwrap());

    set_vm_assigner(None);
}

#[test]
fn main_call_non_zero_return_is_error() {
    let _guard = test_guard();

    let main = main_addr();
    let tx = make_tx(3, main, vec![], 17);
    let mut ctx = make_ctx(
        1,
        &tx,
        Box::new(StateMem::default()),
        Box::new(MemLogs::default()),
    );
    let codes = lang_to_bytecode("return 1").unwrap();

    let err = execute_main_bytecode(&mut ctx, codes).unwrap_err();
    assert!(err.contains("main call return error code 1"), "{err}");
}

#[test]
fn loader_reports_library_index_overflow() {
    let _guard = test_guard();

    let main = main_addr();
    let tx = make_tx(3, main, vec![], 17);
    let mut ctx = make_ctx(
        1,
        &tx,
        Box::new(StateMem::default()),
        Box::new(MemLogs::default()),
    );
    let sig = calc_func_sign("f");
    let codes = single_call_codes(0, sig);

    let err = execute_main_bytecode(&mut ctx, codes).unwrap_err();
    assert!(err.contains("CallLibIdxOverflow"), "{err}");
}

#[test]
fn deploy_rejects_missing_library_contract() {
    let _guard = test_guard();

    let main = main_addr();
    let missing = contract_addr(&main, 7001);
    let tx = make_tx(3, main, vec![], 17);
    let mut ctx = make_ctx(
        1,
        &tx,
        Box::new(StateMem::default()),
        Box::new(MemLogs::default()),
    );
    ctx.env.chain.fast_sync = true;

    let sto = Contract::new()
        .lib(missing.to_addr())
        .func(
            Func::new("run")
                .unwrap()
                .external()
                .fitsh("return 0")
                .unwrap(),
        )
        .into_sto();

    let err = execute_deploy(&mut ctx, 1, sto).unwrap_err();
    assert!(err.contains("library contract"), "{err}");
    assert!(err.contains("does not exist"), "{err}");
}

#[test]
fn deploy_rejects_nested_parent_inherits_before_runtime() {
    let _guard = test_guard();

    let main = main_addr();
    let a = contract_addr(&main, 7101);
    let b = contract_addr(&main, 7102);
    let tx = make_tx(3, main, vec![], 17);
    let mut state = StateMem::default();

    let sto_a = Contract::new()
        .inh(b.to_addr())
        .func(
            Func::new("fa")
                .unwrap()
                .external()
                .fitsh("return 0")
                .unwrap(),
        )
        .into_sto();
    let sto_b = Contract::new()
        .inh(a.to_addr())
        .func(
            Func::new("fb")
                .unwrap()
                .external()
                .fitsh("return 0")
                .unwrap(),
        )
        .into_sto();
    insert_contract(&mut state, &a, &sto_a);
    insert_contract(&mut state, &b, &sto_b);

    let mut ctx = make_ctx(1, &tx, Box::new(state), Box::new(MemLogs::default()));
    ctx.env.chain.fast_sync = true;
    let sto = Contract::new()
        .inh(a.to_addr())
        .func(
            Func::new("run")
                .unwrap()
                .external()
                .fitsh("return 0")
                .unwrap(),
        )
        .into_sto();

    let err = execute_deploy(&mut ctx, 1, sto).unwrap_err();
    assert!(
        err.contains("inherit parent") && err.contains("cannot have parent inherit"),
        "{err}"
    );
}

#[test]
fn deploy_rejects_missing_library_function_at_precheck() {
    let _guard = test_guard();

    let main = main_addr();
    let lib = contract_addr(&main, 7201);
    let tx = make_tx(3, main, vec![], 17);
    let mut state = StateMem::default();

    let lib_sto = Contract::new()
        .func(
            Func::new("g")
                .unwrap()
                .external()
                .fitsh("return 0")
                .unwrap(),
        )
        .into_sto();
    insert_contract(&mut state, &lib, &lib_sto);

    let mut ctx = make_ctx(1, &tx, Box::new(state), Box::new(MemLogs::default()));
    ctx.env.chain.fast_sync = true;
    let sto = Contract::new()
        .lib(lib.to_addr())
        .func(
            Func::new("run")
                .unwrap()
                .external()
                .fitsh(
                    r##"
                    lib C = 0
                    return C.f()
                "##,
                )
                .unwrap(),
        )
        .into_sto();

    let err = execute_deploy(&mut ctx, 1, sto).unwrap_err();
    assert!(err.contains("function"), "{err}");
    assert!(err.contains("not found"), "{err}");
}

#[test]
fn loader_reports_missing_contract() {
    let _guard = test_guard();

    let main = main_addr();
    let missing = contract_addr(&main, 901);
    let tx = make_tx(3, main, vec![missing.to_addr()], 17);
    let mut ctx = make_ctx(
        1,
        &tx,
        Box::new(StateMem::default()),
        Box::new(MemLogs::default()),
    );
    let sig = calc_func_sign("f");
    let codes = single_call_codes(0, sig);

    let err = execute_main_bytecode(&mut ctx, codes).unwrap_err();
    assert!(err.contains("NotFindContract"), "{err}");
}

#[test]
fn loader_enforces_max_loaded_contracts() {
    let _guard = test_guard();

    let main = main_addr();
    let sig = calc_func_sign("f");
    let mut state = StateMem::default();
    let mut addrs = Vec::new();
    let sto = make_external_contract("f", "return 0");
    for i in 0..21u32 {
        let caddr = contract_addr(&main, 1000 + i);
        addrs.push(caddr.to_addr());
        insert_contract(&mut state, &caddr, &sto);
    }

    let tx = make_tx(3, main, addrs, u8::MAX);
    let mut ctx = make_ctx(1, &tx, Box::new(state), Box::new(MemLogs::default()));

    let mut codes = Vec::new();
    for idx in 0..21u8 {
        codes.push(Bytecode::PNIL as u8);
        codes.push(Bytecode::CALLEXT as u8);
        codes.push(idx);
        codes.extend_from_slice(&sig);
    }
    codes.push(Bytecode::END as u8);

    let err = execute_main_bytecode(&mut ctx, codes).unwrap_err();
    assert!(err.contains("OutOfLoadContract"), "{err}");
}

#[test]
fn globals_contract_cache_hits_on_second_machine_call() {
    let _guard = test_guard();

    vm::configure_contract_cache(ContractCacheConfig {
        max_bytes: 2_000_000,
        protected_ratio: 70,
        heat_half_life: 10_000,
        hit_boost: 10,
        promote_threshold: 20,
        max_entry_bytes: 0,
    });

    let main = main_addr();
    let caddr = contract_addr(&main, 2001);
    let sto = make_external_contract("f", "return 0");
    let sig = calc_func_sign("f");
    let codes = single_call_codes(0, sig);

    let mut state = StateMem::default();
    insert_contract(&mut state, &caddr, &sto);

    let pool = vm::global_machine_manager().contract_cache();
    let before = pool.stats();

    let tx = make_tx(3, main, vec![caddr.to_addr()], 17);
    let mut ctx1 = make_ctx(
        1,
        &tx,
        Box::new(state.clone()),
        Box::new(MemLogs::default()),
    );
    execute_main_bytecode(&mut ctx1, codes.clone()).unwrap();
    let mid = pool.stats();

    let mut ctx2 = make_ctx(1, &tx, Box::new(state), Box::new(MemLogs::default()));
    execute_main_bytecode(&mut ctx2, codes).unwrap();
    let after = pool.stats();

    assert!(mid.inserts > before.inserts, "{before:?} -> {mid:?}");
    assert!(after.hits > mid.hits, "{mid:?} -> {after:?}");

    vm::configure_contract_cache(ContractCacheConfig::default());
}

#[test]
fn runtime_log_roundtrip_is_readable() {
    let _guard = test_guard();

    let main = main_addr();
    let tx = make_tx(3, main, vec![], 17);
    let mut ctx = make_ctx(
        1,
        &tx,
        Box::new(StateMem::default()),
        Box::new(MemLogs::default()),
    );
    let codes = lang_to_bytecode(
        r##"
        log(9, 1, 2, 3, 4)
        return 0
    "##,
    )
    .unwrap();

    execute_main_bytecode(&mut ctx, codes).unwrap();

    assert_eq!(ctx.logs().snapshot_len(), 1);
    let raw = ctx.logs().load(1, 0).expect("log item 0 must exist");
    let log = VmLog::build(&raw).expect("log item should decode");

    assert_eq!(log.addr, main);
    assert_eq!(log.topic0.extract_u128().unwrap(), 9);
    assert_eq!(log.topic1.extract_u128().unwrap(), 1);
    assert_eq!(log.topic2.extract_u128().unwrap(), 2);
    assert_eq!(log.topic3.extract_u128().unwrap(), 3);
    assert_eq!(log.data.extract_u128().unwrap(), 4);
}

#[test]
fn sandbox_call_executes_and_reports_missing_function() {
    let _guard = test_guard();

    let main = main_addr();
    let caddr = contract_addr(&main, 3001);
    let sto = make_external_contract(
        "add1",
        r##"
        param { n }
        assert n == 5
        return 0
    "##,
    );
    let mut state = StateMem::default();
    insert_contract(&mut state, &caddr, &sto);

    let tx = make_tx(3, main, vec![], 17);
    let mut ctx = make_ctx(1, &tx, Box::new(state), Box::new(MemLogs::default()));
    protocol::operate::hac_add(&mut ctx, &main, &Amount::unit238(1_000_000_000)).unwrap();

    let callres = machine::sandbox_call(
        &mut ctx,
        machine::SandboxSpec::new(caddr, "add1").args(vec![Value::U8(5)]),
    )
    .unwrap();
    assert!(callres.use_gas > 0);
    assert_eq!(callres.ret_val, Value::U8(0));

    let err = machine::sandbox_call(
        &mut ctx,
        machine::SandboxSpec::new(contract_addr(&main, 3001), "missing").args(vec![Value::U8(5)]),
    )
    .unwrap_err();
    assert!(err.contains("CallNotExist"), "{err}");
}

#[test]
fn sandbox_call_respects_explicit_gas_max_byte() {
    let _guard = test_guard();

    let main = main_addr();
    let caddr = contract_addr(&main, 3002);
    let sto = make_external_contract(
        "heavy",
        r##"
        var i = 0 as u64
        var sum = 0 as u64
        while i < 200 {
            sum = sum + i
            i = i + 1
        }
        return 0
    "##,
    );
    let mut state = StateMem::default();
    insert_contract(&mut state, &caddr, &sto);

    let tx = make_tx(3, main, vec![], 17);
    let mut ctx = make_ctx(1, &tx, Box::new(state), Box::new(MemLogs::default()));
    protocol::operate::hac_add(&mut ctx, &main, &Amount::unit238(1_000_000_000)).unwrap();

    let err = machine::sandbox_call(
        &mut ctx,
        machine::SandboxSpec::new(caddr.clone(), "heavy").gas_max_byte(1),
    )
    .unwrap_err();
    assert!(err.contains("gas has run out"), "{err}");

    let callres = machine::sandbox_call(
        &mut ctx,
        machine::SandboxSpec::new(caddr, "heavy").gas_max_byte(64),
    )
    .unwrap();
    assert!(callres.use_gas > 0);
    assert_eq!(callres.ret_val, Value::U8(0));
}
