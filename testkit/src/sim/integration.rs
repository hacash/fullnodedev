use basis::interface::{Logs, State, TransactionRead};
use field::{Address, Amount};
use protocol::context::ContextInst;
use protocol::transaction::create_tx_info;
use std::sync::{Mutex, MutexGuard, OnceLock};

use crate::sim::context::make_ctx_with_logs;
use crate::sim::tx::{StubTx, StubTxBuilder};

pub fn test_guard() -> MutexGuard<'static, ()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
        .lock()
        .unwrap_or_else(|e| e.into_inner())
}

pub fn vm_main_addr() -> Address {
    Address::from_readable("1MzNY1oA3kfgYi75zquj3SRUPYztzXHzK9").unwrap()
}

pub fn vm_alt_addr() -> Address {
    Address::from_readable("1EuGe2GU8tDKnHLNfBsgyffx66buK7PP6g").unwrap()
}

pub fn make_stub_tx(ty: u8, main: Address, addrs: Vec<Address>, gas_max: u8) -> StubTx {
    StubTxBuilder::new()
        .ty(ty)
        .main(main)
        .addrs(addrs)
        .fee(Amount::unit238(10_000_000))
        .gas_max(gas_max)
        .tx_size(128)
        .fee_purity(3200)
        .build()
}

pub fn make_ctx_from_tx<'a>(
    height: u64,
    tx: &'a dyn TransactionRead,
    state: Box<dyn State>,
    logs: Box<dyn Logs>,
) -> ContextInst<'a> {
    let mut env = basis::component::Env::default();
    env.block.height = height;
    env.tx = create_tx_info(tx);
    make_ctx_with_logs(env, state, logs, tx)
}

pub fn set_vm_assigner(assigner: Option<protocol::setup::FnVmAssignFunc>) {
    unsafe {
        protocol::setup::VM_ASSIGN_FUNC = assigner;
    }
}
