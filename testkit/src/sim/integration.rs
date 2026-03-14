use basis::interface::{Logs, State, TransactionRead};
use field::{Address, Amount};
use protocol::context::ContextInst;
use protocol::setup::{ScopedSetupGuard, SetupBuilder};
use protocol::transaction::create_tx_info;
use std::cell::RefCell;
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

thread_local! {
    static TEST_SETUP_SCOPE: RefCell<Option<ScopedSetupGuard>> = const { RefCell::new(None) };
}

fn build_basic_registry(
    vm_assigner: Option<protocol::setup::FnVmAssignFunc>,
) -> protocol::setup::SetupRegistry {
    let mut builder = SetupBuilder::new()
        .block_hasher(x16rs::block_hash)
        .action_register(protocol::action::register);
    if let Some(assigner) = vm_assigner {
        builder = builder.vm_assigner(assigner);
    }
    builder
        .build()
        .unwrap_or_else(|e| panic!("build scoped setup failed: {}", e))
}

pub fn scoped_setup(vm_assigner: Option<protocol::setup::FnVmAssignFunc>) -> ScopedSetupGuard {
    protocol::setup::install_scoped_for_test(build_basic_registry(vm_assigner))
}

pub fn set_vm_assigner(assigner: Option<protocol::setup::FnVmAssignFunc>) {
    let guard = scoped_setup(assigner);
    TEST_SETUP_SCOPE.with(|cell| {
        *cell.borrow_mut() = Some(guard);
    });
}
