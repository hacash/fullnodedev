/// VM host interface: the interpreter depends only on this trait.
///
/// Design goals:
/// - No `&mut dyn State` / `&mut dyn Logs` are exposed to the interpreter, avoiding long-lived borrows.
/// - Host implementations can enforce runtime policies (allowlists, mode gating, accounting) centrally.
pub trait VmHost {
    fn height(&self) -> u64;
    fn main_entry_bindings(&self) -> FrameBindings;
    fn gas_remaining(&self) -> i64;
    fn gas_charge(&mut self, gas: i64) -> VmrtErr;
    fn contract_edition(&mut self, addr: &ContractAddress) -> Option<ContractEdition>;
    fn contract(&mut self, addr: &ContractAddress) -> Option<ContractSto>;
    fn action_call(&mut self, kid: u16, body: Vec<u8>) -> XRet<(u32, Vec<u8>)>;

    // Logs
    fn log_push(&mut self, addr: &Address, items: Vec<Value>) -> VmrtErr;

    // Storage
    fn srest(&mut self, addr: &Address, key: &Value) -> VmrtRes<Value>;
    fn sload(&mut self, addr: &Address, key: &Value) -> VmrtRes<Value>;
    fn sdel(&mut self, addr: &Address, key: Value) -> VmrtErr;
    fn ssave(
        &mut self,
        gst: &GasExtra,
        addr: &Address,
        key: Value,
        val: Value,
    ) -> VmrtRes<i64>;
    fn srent(
        &mut self,
        gst: &GasExtra,
        addr: &Address,
        key: Value,
        period: Value,
    ) -> VmrtRes<i64>;
}

impl<T: Context + ?Sized> VmHost for T {
    fn height(&self) -> u64 {
        self.env().block.height
    }

    fn main_entry_bindings(&self) -> FrameBindings {
        FrameBindings::root(self.tx().main(), self.env().tx.addrs.clone().into())
    }

    fn gas_remaining(&self) -> i64 {
        Context::gas_remaining(self)
    }

    fn gas_charge(&mut self, gas: i64) -> VmrtErr {
        use crate::rt::ItrErrCode::*;
        if gas < 0 {
            return itr_err_fmt!(GasError, "gas cost invalid: {}", gas);
        }
        Context::gas_charge(self, gas).map_err(|e| ItrErr::new(OutOfGas, &e))
    }

    fn contract_edition(&mut self, addr: &ContractAddress) -> Option<ContractEdition> {
        crate::VMState::wrap(self.state()).contract_edition(addr)
    }

    fn contract(&mut self, addr: &ContractAddress) -> Option<ContractSto> {
        crate::VMState::wrap(self.state()).contract(addr)
    }

    fn action_call(&mut self, kid: u16, body: Vec<u8>) -> XRet<(u32, Vec<u8>)> {
        // ctx.level was already set to the correct call level by setup_vm_run
        Context::action_call(self, kid, body)
    }

    fn log_push(&mut self, addr: &Address, items: Vec<Value>) -> VmrtErr {
        let lgdt = crate::VmLog::new(*addr, items)?;
        self.logs().push(&lgdt);
        Ok(())
    }

    fn srest(&mut self, addr: &Address, key: &Value) -> VmrtRes<Value> {
        let hei = self.env().block.height;
        crate::VMState::wrap(self.state()).srest(hei, addr, key)
    }

    fn sload(&mut self, addr: &Address, key: &Value) -> VmrtRes<Value> {
        let hei = self.env().block.height;
        crate::VMState::wrap(self.state()).sload(hei, addr, key)
    }

    fn sdel(&mut self, addr: &Address, key: Value) -> VmrtErr {
        crate::VMState::wrap(self.state()).sdel(addr, key)
    }

    fn ssave(
        &mut self,
        gst: &GasExtra,
        addr: &Address,
        key: Value,
        val: Value,
    ) -> VmrtRes<i64> {
        let hei = self.env().block.height;
        crate::VMState::wrap(self.state()).ssave(gst, hei, addr, key, val)
    }

    fn srent(
        &mut self,
        gst: &GasExtra,
        addr: &Address,
        key: Value,
        period: Value,
    ) -> VmrtRes<i64> {
        let hei = self.env().block.height;
        crate::VMState::wrap(self.state()).srent(gst, hei, addr, key, period)
    }
}
