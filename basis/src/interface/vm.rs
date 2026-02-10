
pub trait VM {
    fn usable(&self) -> bool { false }
    fn call(&mut self, _: &mut dyn Context, _: u8, _: u8, _: &[u8], _: Box<dyn Any>)
        -> Ret<(i64, Vec<u8>)> { never!() }
    /// Snapshot volatile VM state (global_vals, memory_vals, gas remaining)
    /// for rollback in AstSelect/AstIf recover paths.
    fn snapshot_volatile(&self) -> Box<dyn Any> { Box::new(()) }
    /// Restore volatile VM state from a previous snapshot.
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



