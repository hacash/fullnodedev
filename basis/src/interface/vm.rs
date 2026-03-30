pub trait VM {
    fn call(&mut self, ctx: &mut dyn Context, req: Box<dyn Any>) -> XRet<(GasUse, Box<dyn Any>)> {
        let _ = (ctx, req);
        never!()
    }
    fn current_intent_scope(&mut self) -> Option<Option<usize>> { None }
    /// Snapshot volatile VM state for AstSelect/AstIf recover paths.
    /// Note: gas remaining is intentionally excluded so gas usage stays monotonic in one tx.
    fn snapshot_volatile(&mut self) -> Box<dyn Any> { Box::new(()) }
    /// Restore volatile VM state from a previous snapshot (excluding gas remaining).
    fn restore_volatile(&mut self, _: Box<dyn Any>) {}
    /// Cross-generation fallback restore used when snapshot side had no VM but current side has initialized VM.
    /// Must rollback branch-local volatile state while preserving warmup/cache/gas monotonic channels.
    fn rollback_volatile_preserve_warm_and_gas(&mut self) {}
    /// Invalidate contract caches by address (global and tx-local, if implementation supports it).
    fn invalidate_contract_cache(&mut self, _: &Address) {}
    /// Snapshot runtime config (e.g. GasExtra/SpaceCap) if implementation supports it.
    fn runtime_config(&mut self) -> Option<Box<dyn Any>> { None }
    /// Execute deferred VM callbacks at tx tail.
    fn drain_deferred(&mut self, _: &mut dyn Context) -> Rerr { Ok(()) }
}
