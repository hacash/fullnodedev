
pub trait VM {
    fn usable(&self) -> bool { false }
    fn call(&mut self, _: &mut dyn Context, _: u8, _: u8, _: &[u8], _: Box<dyn Any>)
        -> Ret<(i64, Vec<u8>)> { never!() }
    /// Snapshot volatile VM state for AstSelect/AstIf recover paths.
    /// Note: gas remaining is intentionally excluded so gas usage stays monotonic in one tx.
    fn snapshot_volatile(&self) -> Box<dyn Any> { Box::new(()) }
    /// Restore volatile VM state from a previous snapshot (excluding gas remaining).
    fn restore_volatile(&mut self, _: Box<dyn Any>) {}
}


pub struct VMNil {}
impl VM for VMNil {}

impl VMNil {
    pub fn new() -> Self {
        VMNil{}
    }

    pub fn empty() -> Box<dyn VM> {
        Box::new(VMNil::new())
    }
}


