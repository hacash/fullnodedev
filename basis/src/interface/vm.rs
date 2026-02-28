
pub struct VMCall<'a> {
    pub ctx: &'a mut dyn Context,
    pub mode: u8,
    pub kind: u8,
    pub payload: Arc<[u8]>,
    pub param: Box<dyn Any>,
}

impl<'a> VMCall<'a> {
    pub fn new(
        ctx: &'a mut dyn Context,
        mode: u8,
        kind: u8,
        payload: Arc<[u8]>,
        param: Box<dyn Any>,
    ) -> Self {
        Self {
            ctx,
            mode,
            kind,
            payload,
            param,
        }
    }
}

pub trait VM {
    fn is_nil(&self) -> bool { false }
    fn call(&mut self, _: VMCall<'_>)
        -> BRet<(i64, Vec<u8>)> { never!() }
    /// Snapshot volatile VM state for AstSelect/AstIf recover paths.
    /// Note: gas remaining is intentionally excluded so gas usage stays monotonic in one tx.
    fn snapshot_volatile(&self) -> Box<dyn Any> { Box::new(()) }
    /// Restore volatile VM state from a previous snapshot (excluding gas remaining).
    fn restore_volatile(&mut self, _: Box<dyn Any>) {}
    /// Cross-generation fallback restore used when snapshot side had VMNil but current side has initialized VM.
    /// Must rollback branch-local volatile state while preserving warmup/cache/gas monotonic channels.
    fn restore_but_keep_warmup(&mut self) {}
    /// Invalidate contract caches by address (global and tx-local, if implementation supports it).
    fn invalidate_contract_cache(&mut self, _: &Address) {}
}


pub struct VMNil {}
impl VM for VMNil {
    fn is_nil(&self) -> bool { true }
}

impl VMNil {
    pub fn new() -> Self {
        VMNil{}
    }

    pub fn empty() -> Box<dyn VM> {
        Box::new(VMNil::new())
    }
}
