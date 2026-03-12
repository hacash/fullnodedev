

pub trait StateOperat {
    fn state(&mut self) -> &mut dyn State;
    fn state_fork(&mut self) -> Arc<Box<dyn State>>;
    fn state_merge(&mut self, _: Arc<Box<dyn State>>);
    fn state_recover(&mut self, _: Arc<Box<dyn State>>);
}

pub trait VmGasMut {
    fn gas_remaining_mut(&mut self) -> Ret<&mut i64>;
}

pub trait Context : StateOperat {
    /// Reset per-transaction caches/state inside Context.
    /// This must be called whenever the underlying tx/env is replaced for a new transaction.
    fn reset_for_new_tx(&mut self, _: &dyn TransactionRead);
    fn action_call(&mut self, _: u16, _: Vec<u8>) -> XRet<(u32, Vec<u8>)>;
    fn action_exec_from(&self) -> ActExecFrom { ActExecFrom::TxLoop }
    fn action_exec_from_set(&mut self, _: ActExecFrom) {}
    fn env(&self) -> &Env;
    fn addr(&self, _:&AddrOrPtr) -> Ret<Address>;
    fn check_sign(&mut self, _: &Address) -> Rerr;
    fn level(&self) -> usize;
    fn level_set(&mut self, _: usize);
    fn tx(&self) -> &dyn TransactionRead;
    fn vm(&mut self) -> &mut dyn VM;
    fn vm_init_once(&mut self, _: Box<dyn VM>) -> Rerr;
    fn gas_init_tx(&mut self, _: i64, _: i64) -> Rerr { errf!("context gas init not supported") }
    fn gas_refund(&mut self) -> Rerr { errf!("context gas refund not supported") }
    fn gas_remaining(&self) -> i64 { 0 }
    fn gas_consume(&mut self, _: u32) -> Rerr { Ok(()) }
    fn vm_gas_mut(&mut self) -> Ret<&mut dyn VmGasMut> { errf!("context vm gas mutable access not supported") }
    fn snapshot_volatile(&self) -> Box<dyn Any> { Box::new(()) }
    fn restore_volatile(&mut self, _: Box<dyn Any>) {}
    // tex
    fn tex_ledger(&mut self) -> &mut TexLedger;
    // log
    fn logs(&mut self) -> &mut dyn Logs;
    // p2sh
    fn p2sh(&self, _: &Address) -> Ret<&dyn P2sh> { errf!("not found") }
    fn p2sh_set(&mut self, _: Address, _: Box<dyn P2sh>) -> Rerr { Ok(()) }
}
