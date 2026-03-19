pub trait StateOperat {
    fn state(&mut self) -> &mut dyn State;
    fn state_fork(&mut self) -> Arc<Box<dyn State>>;
    fn state_merge(&mut self, _: Arc<Box<dyn State>>);
    fn state_recover(&mut self, _: Arc<Box<dyn State>>);
}

pub trait Context: StateOperat {
    fn action_call(&mut self, _: u16, _: Vec<u8>) -> XRet<(u32, Vec<u8>)>;
    fn exec_from(&self) -> ExecFrom { ExecFrom::Top }
    fn exec_from_set(&mut self, _: ExecFrom) {}
    fn env(&self) -> &Env;
    fn addr(&self, _: &AddrOrPtr) -> Ret<Address>;
    fn check_sign(&mut self, _: &Address) -> Rerr;
    fn tx(&self) -> &dyn TransactionRead;
    fn vm_call(&mut self, req: Box<dyn Any>) -> XRet<(i64, Box<dyn Any>)>;
    fn vm_snapshot_volatile(&mut self) -> Option<Box<dyn Any>> { None }
    fn vm_restore_volatile(&mut self, _: Box<dyn Any>) {}
    fn vm_restore_but_keep_warmup(&mut self) {}
    fn vm_invalidate_contract_cache(&mut self, _: &Address) {}
    fn gas_remaining(&self) -> i64 { 0  }
    fn gas_charge(&mut self, _: i64) -> Rerr { Ok(()) }
    fn gas_initialize(&mut self, _: i64) -> Rerr {
        errf!("context gas init not supported")
    }
    fn gas_refund(&mut self) -> Rerr {
        errf!("context gas refund not supported")
    }
    fn snapshot_volatile(&self) -> Box<dyn Any> { Box::new(()) }
    fn restore_volatile(&mut self, _: Box<dyn Any>) {}
    // tex
    fn tex_ledger(&mut self) -> &mut TexLedger;
    // log
    fn logs(&mut self) -> &mut dyn Logs;
    // p2sh
    fn p2sh(&self, _: &Address) -> Ret<&dyn P2sh> {  errf!("not found") }
    fn p2sh_set(&mut self, _: Address, _: Box<dyn P2sh>) -> Rerr { Ok(()) }
}

pub struct ExecFromGuard<'a> {
    ctx: &'a mut dyn Context,
    old_exec_from: ExecFrom,
}

impl ExecFromGuard<'_> {
    pub fn ctx(&mut self) -> &mut dyn Context {
        self.ctx
    }
}

impl Drop for ExecFromGuard<'_> {
    fn drop(&mut self) {
        self.ctx.exec_from_set(self.old_exec_from);
    }
}

pub fn enter_exec_from<'a>(ctx: &'a mut dyn Context, from: ExecFrom) -> ExecFromGuard<'a> {
    let old_exec_from = ctx.exec_from();
    ctx.exec_from_set(from);
    ExecFromGuard { ctx, old_exec_from }
}

pub fn with_exec_from<T>(
    ctx: &mut dyn Context,
    from: ExecFrom,
    f: impl for<'a> FnOnce(&'a mut dyn Context) -> T,
) -> T {
    let mut guard = enter_exec_from(ctx, from);
    f(guard.ctx())
}
