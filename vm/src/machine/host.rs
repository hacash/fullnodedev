/// VM host interface: the interpreter depends only on this trait.
///
/// Design goals:
/// - No `&mut dyn State` / `&mut dyn Logs` are exposed to the interpreter, avoiding long-lived borrows.
/// - Host implementations can enforce runtime policies (allowlists, mode gating, accounting) centrally.
pub trait VmHost {
    fn height(&mut self) -> u64;
    fn action_call(&mut self, kid: u16, body: Vec<u8>) -> XRet<(u32, Vec<u8>)>;

    // Logs
    fn log_push(&mut self, addr: &Address, items: Vec<Value>) -> VmrtErr;

    // Storage
    fn srest(&mut self, hei: u64, addr: &Address, key: &Value) -> VmrtRes<Value>;
    fn sload(&mut self, hei: u64, addr: &Address, key: &Value) -> VmrtRes<Value>;
    fn sdel(&mut self, addr: &Address, key: Value) -> VmrtErr;
    fn ssave(
        &mut self,
        gst: &GasExtra,
        hei: u64,
        addr: &Address,
        key: Value,
        val: Value,
    ) -> VmrtRes<i64>;
    fn srent(
        &mut self,
        gst: &GasExtra,
        hei: u64,
        addr: &Address,
        key: Value,
        period: Value,
    ) -> VmrtRes<i64>;
}

/// Adapter: provide `VmHost` on top of an existing `Context`.
pub struct CtxHost<'a> {
    pub ctx: &'a mut dyn Context,
}

impl<'a> CtxHost<'a> {
    pub fn new(ctx: &'a mut dyn Context) -> Self {
        Self { ctx }
    }
}

impl VmHost for CtxHost<'_> {
    fn height(&mut self) -> u64 {
        self.ctx.env().block.height
    }

    fn action_call(&mut self, kid: u16, body: Vec<u8>) -> XRet<(u32, Vec<u8>)> {
        // ctx.level was already set to the correct call level by setup_vm_run
        self.ctx.action_call(kid, body)
    }

    fn log_push(&mut self, addr: &Address, items: Vec<Value>) -> VmrtErr {
        let lgdt = crate::VmLog::new(*addr, items)?;
        let logs: &mut dyn Logs = self.ctx.logs();
        logs.push(&lgdt);
        Ok(())
    }

    fn srest(&mut self, hei: u64, addr: &Address, key: &Value) -> VmrtRes<Value> {
        let mut st = crate::VMState::wrap(self.ctx.state());
        st.srest(hei, addr, key)
    }

    fn sload(&mut self, hei: u64, addr: &Address, key: &Value) -> VmrtRes<Value> {
        let mut st = crate::VMState::wrap(self.ctx.state());
        st.sload(hei, addr, key)
    }

    fn sdel(&mut self, addr: &Address, key: Value) -> VmrtErr {
        let mut st = crate::VMState::wrap(self.ctx.state());
        st.sdel(addr, key)
    }

    fn ssave(
        &mut self,
        gst: &GasExtra,
        hei: u64,
        addr: &Address,
        key: Value,
        val: Value,
    ) -> VmrtRes<i64> {
        let mut st = crate::VMState::wrap(self.ctx.state());
        st.ssave(gst, hei, addr, key, val)
    }

    fn srent(
        &mut self,
        gst: &GasExtra,
        hei: u64,
        addr: &Address,
        key: Value,
        period: Value,
    ) -> VmrtRes<i64> {
        let mut st = crate::VMState::wrap(self.ctx.state());
        st.srent(gst, hei, addr, key, period)
    }
}
