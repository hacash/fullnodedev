use crate::machine::{DeferredRegistry, VmHost};
use basis::component::Env;
use basis::interface::{Context, TransactionRead};
use field::{Address, Amount, Hash};
use protocol::context::ContextInst;
use protocol::state::EmptyLogs;
use space::{CtcKVMap, GKVMap, Heap, Stack};
use sys::Ret;

pub type VmTestExecRes = (CallExit, i64, Vec<Value>, Heap);

struct TestVmHost<'a> {
    ctx: &'a mut dyn Context,
    gas_remaining: i64,
}

impl VmHost for TestVmHost<'_> {
    fn height(&self) -> u64 {
        self.ctx.env().block.height
    }

    fn main_entry_bindings(&self) -> FrameBindings {
        FrameBindings::root(self.ctx.tx().main(), self.ctx.env().tx.addrs.clone().into())
    }

    fn gas_remaining(&self) -> i64 {
        self.gas_remaining
    }

    fn gas_charge(&mut self, gas: i64) -> VmrtErr {
        if gas < 0 {
            return itr_err_fmt!(GasError, "gas cost invalid: {}", gas);
        }
        self.gas_remaining -= gas;
        if self.gas_remaining < 0 {
            return itr_err_code!(OutOfGas);
        }
        Ok(())
    }

    fn contract_edition(&mut self, addr: &ContractAddress) -> Option<ContractEdition> {
        crate::VMState::wrap(self.ctx.state()).contract_edition(addr)
    }

    fn contract(&mut self, addr: &ContractAddress) -> Option<ContractSto> {
        crate::VMState::wrap(self.ctx.state()).contract(addr)
    }

    fn action_call(&mut self, kid: u16, body: Vec<u8>) -> XRet<(u32, Vec<u8>)> {
        self.ctx.action_call(kid, body)
    }

    fn log_push(&mut self, addr: &Address, items: Vec<Value>) -> VmrtErr {
        let lgdt = crate::VmLog::new(*addr, items)?;
        self.ctx.logs().push(&lgdt);
        Ok(())
    }

    fn srest(&mut self, addr: &Address, key: &Value) -> VmrtRes<Value> {
        let hei = self.ctx.env().block.height;
        crate::VMState::wrap(self.ctx.state()).srest(hei, addr, key)
    }

    fn sload(&mut self, addr: &Address, key: &Value) -> VmrtRes<Value> {
        let hei = self.ctx.env().block.height;
        crate::VMState::wrap(self.ctx.state()).sload(hei, addr, key)
    }

    fn sdel(&mut self, addr: &Address, key: Value) -> VmrtErr {
        crate::VMState::wrap(self.ctx.state()).sdel(addr, key)
    }

    fn ssave(&mut self, gst: &GasExtra, addr: &Address, key: Value, val: Value) -> VmrtRes<i64> {
        let hei = self.ctx.env().block.height;
        crate::VMState::wrap(self.ctx.state()).ssave(gst, hei, addr, key, val)
    }

    fn srent(&mut self, gst: &GasExtra, addr: &Address, key: Value, period: Value) -> VmrtRes<i64> {
        let hei = self.ctx.env().block.height;
        crate::VMState::wrap(self.ctx.state()).srent(gst, hei, addr, key, period)
    }
}

#[derive(Default, Clone, Debug)]
struct DummyTx;

impl field::Serialize for DummyTx {
    fn size(&self) -> usize {
        0
    }
    fn serialize(&self) -> Vec<u8> {
        vec![]
    }
}

impl basis::interface::TxExec for DummyTx {}

impl TransactionRead for DummyTx {
    fn ty(&self) -> u8 {
        3
    }
    fn hash(&self) -> Hash {
        Hash::default()
    }
    fn hash_with_fee(&self) -> Hash {
        Hash::default()
    }
    fn main(&self) -> Address {
        Address::default()
    }
    fn addrs(&self) -> Vec<Address> {
        vec![Address::default()]
    }
    fn fee(&self) -> &Amount {
        Amount::zero_ref()
    }
    fn fee_purity(&self) -> u64 {
        1
    }
    fn gas_max_byte(&self) -> Option<u8> {
        Some(1)
    }
}

#[allow(dead_code)]
fn execute_test_maincall(gas: i64, codes: Vec<u8>) -> VmrtRes<VmTestExecRes> {
    execute_test_with_argv(gas, codes, None)
}

#[allow(dead_code)]
fn execute_test_with_argv(
    gas_limit: i64,
    codes: Vec<u8>,
    argv: Option<Value>,
) -> VmrtRes<VmTestExecRes> {
    let mut pc: usize = 0;
    let gas: i64 = gas_limit; // 2000
    // let addr = Address::from_readable("1MzNY1oA3kfgYi75zquj3SRUPYztzXHzK9").unwrap();
    let cadr = ContractAddress::default();

    let tx = DummyTx::default();
    let mut env = Env::default();
    env.block.height = 1;
    let mut ctx = ContextInst::new(
        env,
        Box::new(StateMem::default()),
        Box::new(EmptyLogs {}),
        &tx,
    );
    let ctx: &mut dyn Context = &mut ctx;

    let mut ops = Stack::new(256);
    if let Some(v) = argv {
        ops.push(v).unwrap();
    }

    let mut heap = Heap::new(64);

    // do execute
    let mut host = TestVmHost {
        ctx,
        gas_remaining: gas,
    };
    let mut gas_use = basis::interface::GasUse::default();
    let mut deferred_registry = DeferredRegistry::default();
    super::interpreter::execute_code(
        &mut pc,
        &codes,
        ExecCtx::main(),
        &mut ops,
        &mut Stack::new(256),
        &mut heap,
        &cadr,
        &cadr,
        &GasTable::new(1),
        &GasExtra::new(1),
        &SpaceCap::new(1),
        &mut gas_use,
        &mut GKVMap::new(20),
        &mut CtcKVMap::new(12),
        &mut deferred_registry,
        &mut host,
    )
    .map(|r| (r, gas_limit - host.gas_remaining, ops.release(), heap))
}
