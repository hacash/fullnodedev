
use basis::component::Env;
use basis::interface::{Context, TransactionRead};
use field::{Address, Amount, Hash};
use protocol::context::ContextInst;
use protocol::state::EmptyLogs;
use space::{CtcKVMap, GKVMap, Heap, Stack};
use sys::Ret;
use crate::machine::CtxHost;

pub type VmTestExecRes = (CallExit, i64, Vec<Value>, Heap);

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
    fn fee_extend(&self) -> Ret<u8> { Ok(1) }
}

#[allow(dead_code)]
fn execute_test_maincall(gas: i64, codes: Vec<u8>) -> VmrtRes<VmTestExecRes> {
    execute_test_with_argv(gas, codes, None)
}


#[allow(dead_code)]
fn execute_test_with_argv(gas_limit: i64, codes: Vec<u8>, argv: Option<Value>) -> VmrtRes<VmTestExecRes> {

    let mut pc: usize = 0;
    let mut gas: i64 = gas_limit; // 2000
    // let addr = Address::from_readable("1MzNY1oA3kfgYi75zquj3SRUPYztzXHzK9").unwrap();
    let cadr = ContractAddress::default();

    let tx = DummyTx::default();
    let mut env = Env::default();
    env.block.height = 1;
    let mut ctx = ContextInst::new(env, Box::new(StateMem::default()), Box::new(EmptyLogs{}), &tx);
    let ctx: &mut dyn Context = &mut ctx;

    let mut ops = Stack::new(256);
    if let Some(v) = argv {
        ops.push(v).unwrap();
    }

    let mut heap = Heap::new(64);

    // do execute
    let mut host = CtxHost::new(ctx);
    super::interpreter::execute_code(
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
