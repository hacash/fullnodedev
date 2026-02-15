use std::collections::HashMap;
use std::sync::{Arc, Mutex, MutexGuard, OnceLock};

use basis::component::Env;
use basis::interface::{Context, Logs, State, TransactionRead};
use field::{Address, Amount, Field, Hash, Serialize, Uint4};
use protocol::context::ContextInst;
use protocol::transaction::create_tx_info;
use sys::Ret;
use vm::contract::{Contract, Func};
use vm::frame::ExecEnv;
use vm::lang::lang_to_bytecode;
use vm::machine::{self, ContractCacheConfig, Machine, Resoure};
use vm::rt::{Bytecode, CodeType, ExecMode, FnSign, calc_func_sign};
use vm::value::Value;
use vm::{ContractAddress, ContractSto, VMState, VmLog};

#[derive(Default, Clone)]
struct StateMem {
    mem: HashMap<Vec<u8>, Vec<u8>>,
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

#[derive(Default, Clone)]
struct MemLogs {
    items: Vec<Vec<u8>>,
}

impl Logs for MemLogs {
    fn push(&mut self, stuff: &dyn Serialize) {
        self.items.push(stuff.serialize());
    }

    fn load(&self, _height: u64, idx: usize) -> Option<Vec<u8>> {
        self.items.get(idx).cloned()
    }

    fn remove(&self, _height: u64) {
        // no-op for in-memory test logs
    }

    fn snapshot_len(&self) -> usize {
        self.items.len()
    }

    fn truncate(&mut self, len: usize) {
        self.items.truncate(len);
    }
}

#[derive(Clone, Debug)]
struct TestTx {
    ty: u8,
    main: Address,
    addrs: Vec<Address>,
    fee: Amount,
    gas_max: u8,
    tx_size: usize,
}

impl Serialize for TestTx {
    fn size(&self) -> usize {
        self.tx_size
    }

    fn serialize(&self) -> Vec<u8> {
        vec![]
    }
}

impl basis::interface::TxExec for TestTx {}

impl TransactionRead for TestTx {
    fn ty(&self) -> u8 {
        self.ty
    }

    fn hash(&self) -> Hash {
        Hash::default()
    }

    fn hash_with_fee(&self) -> Hash {
        Hash::default()
    }

    fn main(&self) -> Address {
        self.main
    }

    fn addrs(&self) -> Vec<Address> {
        self.addrs.clone()
    }

    fn fee(&self) -> &Amount {
        &self.fee
    }

    fn fee_got(&self) -> Amount {
        self.fee.clone()
    }

    fn fee_purity(&self) -> u64 {
        3200
    }

    fn fee_extend(&self) -> Ret<u8> {
        Ok(self.gas_max)
    }
}

fn test_guard() -> MutexGuard<'static, ()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
        .lock()
        .unwrap_or_else(|e| e.into_inner())
}

fn main_addr() -> Address {
    Address::from_readable("1MzNY1oA3kfgYi75zquj3SRUPYztzXHzK9").unwrap()
}

fn contract_addr(main: &Address, nonce: u32) -> ContractAddress {
    ContractAddress::calculate(main, &Uint4::from(nonce))
}

fn make_tx(ty: u8, main: Address, addrs: Vec<Address>, gas_max: u8) -> TestTx {
    TestTx {
        ty,
        main,
        addrs,
        fee: Amount::unit238(10_000_000),
        gas_max,
        tx_size: 128,
    }
}

fn make_ctx<'a>(height: u64, tx: &'a TestTx, state: Box<dyn State>, logs: Box<dyn Logs>) -> ContextInst<'a> {
    let mut env = Env::default();
    env.block.height = height;
    env.tx = create_tx_info(tx);
    ContextInst::new(env, state, logs, tx)
}

fn insert_contract(state: &mut dyn State, addr: &ContractAddress, sto: &ContractSto) {
    let mut vm_state = VMState::wrap(state);
    vm_state.contract_set(addr, sto);
}

fn make_public_contract(func_name: &str, body: &str) -> ContractSto {
    Contract::new()
        .func(Func::new(func_name).unwrap().public().fitsh(body).unwrap())
        .into_sto()
}

fn execute_main_bytecode(ctx: &mut dyn Context, codes: Vec<u8>) -> Ret<Value> {
    let mut gas = 1_000_000i64;
    let height = ctx.env().block.height;
    let mut machine = Machine::create(Resoure::create(height));
    let mut exenv = ExecEnv {
        ctx,
        gas: &mut gas,
    };
    machine.main_call(&mut exenv, CodeType::Bytecode, codes.into())
}

fn single_call_codes(lib_idx: u8, sig: FnSign) -> Vec<u8> {
    let mut codes = vec![Bytecode::PNIL as u8, Bytecode::CALL as u8, lib_idx];
    codes.extend_from_slice(&sig);
    codes.push(Bytecode::END as u8);
    codes
}

fn set_vm_assigner(assigner: Option<protocol::setup::FnVmAssignFunc>) {
    unsafe {
        protocol::setup::VM_ASSIGN_FUNC = assigner;
    }
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

    let err = machine::setup_vm_run(
        &mut ctx,
        ExecMode::Main as u8,
        CodeType::Bytecode as u8,
        Arc::from(vec![Bytecode::END as u8]),
        Value::Nil,
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

    let err = machine::setup_vm_run(
        &mut ctx,
        ExecMode::Main as u8,
        CodeType::Bytecode as u8,
        Arc::from(vec![Bytecode::END as u8]),
        Value::Nil,
    )
    .unwrap_err();

    assert!(err.contains("vm not initialized"), "{err}");
}

#[test]
fn setup_vm_run_executes_after_assigner_registered() {
    let _guard = test_guard();
    set_vm_assigner(Some(machine::vm_assign));

    let main = main_addr();
    let tx = make_tx(3, main, vec![], 17);
    let mut ctx = make_ctx(
        1,
        &tx,
        Box::new(StateMem::default()),
        Box::new(MemLogs::default()),
    );
    protocol::operate::hac_add(&mut ctx, &main, &Amount::unit238(1_000_000_000)).unwrap();

    let (gas_used, rv) = machine::setup_vm_run(
        &mut ctx,
        ExecMode::Main as u8,
        CodeType::Bytecode as u8,
        Arc::from(vec![Bytecode::END as u8]),
        Value::Nil,
    )
    .unwrap();

    assert!(gas_used > 0);
    assert!(!rv.check_true());

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
    assert!(err.contains("CallViewOverflow"), "{err}");
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
    let sto = make_public_contract("f", "return 0");
    for i in 0..21u32 {
        let caddr = contract_addr(&main, 1000 + i);
        addrs.push(caddr.to_addr());
        insert_contract(&mut state, &caddr, &sto);
    }

    let tx = make_tx(3, main, addrs, 17);
    let mut ctx = make_ctx(1, &tx, Box::new(state), Box::new(MemLogs::default()));

    let mut codes = Vec::new();
    for idx in 0..21u8 {
        codes.push(Bytecode::PNIL as u8);
        codes.push(Bytecode::CALL as u8);
        codes.push(idx);
        codes.extend_from_slice(&sig);
    }
    codes.push(Bytecode::END as u8);

    let err = execute_main_bytecode(&mut ctx, codes).unwrap_err();
    assert!(err.contains("OutOfLoadContract"), "{err}");
}

#[test]
fn global_contract_cache_hits_on_second_machine_call() {
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
    let sto = make_public_contract("f", "return 0");
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
    assert_eq!(log.topic0.to_uint(), 9);
    assert_eq!(log.topic1.to_uint(), 1);
    assert_eq!(log.topic2.to_uint(), 2);
    assert_eq!(log.topic3.to_uint(), 3);
    assert_eq!(log.data.to_uint(), 4);
}

#[test]
fn sandbox_call_executes_and_reports_missing_function() {
    let _guard = test_guard();

    let main = main_addr();
    let caddr = contract_addr(&main, 3001);
    let sto = make_public_contract(
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

    let (gas, ret_json) = machine::sandbox_call(&mut ctx, caddr, "add1".to_owned(), "5:u8").unwrap();
    let ret: serde_json::Value = serde_json::from_str(&ret_json).unwrap();
    assert!(gas > 0);
    assert_eq!(ret.as_u64(), Some(0));

    let err = machine::sandbox_call(
        &mut ctx,
        contract_addr(&main, 3001),
        "missing".to_owned(),
        "5:u8",
    )
    .unwrap_err();
    assert!(err.contains("CallNotExist"), "{err}");
}
