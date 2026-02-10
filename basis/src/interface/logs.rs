

pub trait Logs : Send + Sync {
    fn push(&mut self, _ :&dyn Serialize) {}
    fn load(&self, _: u64, _: usize) -> Option<Vec<u8>> { None }
    fn remove(&self, _: u64) {}
    //
    fn height(&self) -> u64 { 0 }
    fn write_to_disk(&self) {}
    /// Return current log count for snapshot before AstSelect/AstIf fork.
    fn snapshot_len(&self) -> usize { 0 }
    /// Truncate logs back to a previous snapshot length on recover.
    fn truncate(&mut self, _len: usize) {}
}