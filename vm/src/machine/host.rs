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
    fn gas_rebate(&mut self, gas: i64) -> VmrtErr;
    fn contract_edition(&mut self, addr: &ContractAddress) -> Option<ContractEdition>;
    fn contract(&mut self, addr: &ContractAddress) -> Option<ContractSto>;
    fn action_call(&mut self, kid: u16, body: Vec<u8>) -> XRet<(u32, Vec<u8>)>;

    #[cfg(feature = "calcfunc")]
    fn calc_call(
        &mut self,
        owner: &ContractAddress,
        selector: FnSign,
        calcfn: &CalcFnObj,
        input: Vec<u8>,
        gas_limit: i64,
    ) -> VmrtRes<(i64, Vec<u8>)> {
        let _ = (owner, selector, calcfn, input, gas_limit);
        itr_err_fmt!(InstDisabled, "calcfunc executor not configured")
    }

    // Logs
    fn log_push(&mut self, addr: &Address, items: Vec<Value>) -> VmrtErr;

    // Storage
    fn sstat(&mut self, gst: &GasExtra, cap: &SpaceCap, addr: &Address, key: &Value) -> VmrtRes<Value>;
    fn sload(&mut self, gst: &GasExtra, cap: &SpaceCap, addr: &Address, key: &Value) -> VmrtRes<Value>;
    fn sdel(&mut self, gst: &GasExtra, cap: &SpaceCap, addr: &Address, key: Value) -> VmrtRes<i64>;
    fn snew(
        &mut self,
        gst: &GasExtra,
        cap: &SpaceCap,
        addr: &Address,
        key: Value,
        val: Value,
        period: Value,
    ) -> VmrtRes<i64>;
    fn sedit(
        &mut self,
        gst: &GasExtra,
        cap: &SpaceCap,
        addr: &Address,
        key: Value,
        val: Value,
    ) -> VmrtRes<(i64, i64)>;
    fn srent(
        &mut self,
        gst: &GasExtra,
        cap: &SpaceCap,
        addr: &Address,
        key: Value,
        period: Value,
    ) -> VmrtRes<i64>;
    fn srecv(
        &mut self,
        gst: &GasExtra,
        cap: &SpaceCap,
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
        Context::gas_charge(self, gas).map_err(|e| ItrErr::new(OutOfGas, &e))
    }

    fn gas_rebate(&mut self, gas: i64) -> VmrtErr {
        use crate::rt::ItrErrCode::*;
        Context::gas_rebate(self, gas).map_err(|e| ItrErr::new(GasError, &e))
    }

    fn contract_edition(&mut self, addr: &ContractAddress) -> Option<ContractEdition> {
        crate::VMState::wrap(self.state()).contract_edition(addr)
    }

    fn contract(&mut self, addr: &ContractAddress) -> Option<ContractSto> {
        crate::VMState::wrap(self.state()).contract(addr)
    }

    fn action_call(&mut self, kid: u16, body: Vec<u8>) -> XRet<(u32, Vec<u8>)> {
        // setup_vm_run already switched the context into CALL scope
        Context::action_call(self, kid, body)
    }

    fn log_push(&mut self, addr: &Address, items: Vec<Value>) -> VmrtErr {
        let lgdt = crate::VmLog::new(*addr, items)?;
        self.logs().push(&lgdt);
        Ok(())
    }

    fn sstat(&mut self, gst: &GasExtra, cap: &SpaceCap, addr: &Address, key: &Value) -> VmrtRes<Value> {
        let hei = self.env().block.height;
        crate::VMState::wrap(self.state()).sstat(gst, cap, hei, addr, key)
    }

    fn sload(&mut self, gst: &GasExtra, cap: &SpaceCap, addr: &Address, key: &Value) -> VmrtRes<Value> {
        let hei = self.env().block.height;
        crate::VMState::wrap(self.state()).sload(gst, cap, hei, addr, key)
    }

    fn sdel(&mut self, gst: &GasExtra, cap: &SpaceCap, addr: &Address, key: Value) -> VmrtRes<i64> {
        let hei = self.env().block.height;
        crate::VMState::wrap(self.state()).sdel(gst, cap, hei, addr, key)
    }

    fn snew(
        &mut self,
        gst: &GasExtra,
        cap: &SpaceCap,
        addr: &Address,
        key: Value,
        val: Value,
        period: Value,
    ) -> VmrtRes<i64> {
        let hei = self.env().block.height;
        crate::VMState::wrap(self.state()).snew(gst, cap, hei, addr, key, val, period)
    }

    fn sedit(
        &mut self,
        gst: &GasExtra,
        cap: &SpaceCap,
        addr: &Address,
        key: Value,
        val: Value,
    ) -> VmrtRes<(i64, i64)> {
        let hei = self.env().block.height;
        crate::VMState::wrap(self.state()).sedit(gst, cap, hei, addr, key, val)
    }

    fn srent(
        &mut self,
        gst: &GasExtra,
        cap: &SpaceCap,
        addr: &Address,
        key: Value,
        period: Value,
    ) -> VmrtRes<i64> {
        let hei = self.env().block.height;
        crate::VMState::wrap(self.state()).srent(gst, cap, hei, addr, key, period)
    }

    fn srecv(
        &mut self,
        gst: &GasExtra,
        cap: &SpaceCap,
        addr: &Address,
        key: Value,
        period: Value,
    ) -> VmrtRes<i64> {
        let hei = self.env().block.height;
        crate::VMState::wrap(self.state()).srecv(gst, cap, hei, addr, key, period)
    }
}
