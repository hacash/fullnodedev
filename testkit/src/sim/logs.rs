use basis::interface::Logs;
use field::Serialize;

#[derive(Default, Clone)]
pub struct MemLogs {
    entries: Vec<Vec<u8>>,
}

impl MemLogs {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

impl Logs for MemLogs {
    fn push(&mut self, stuff: &dyn Serialize) {
        self.entries.push(stuff.serialize());
    }

    fn load(&self, _height: u64, idx: usize) -> Option<Vec<u8>> {
        self.entries.get(idx).cloned()
    }

    fn remove(&self, _height: u64) {}

    fn snapshot_len(&self) -> usize {
        self.entries.len()
    }

    fn truncate(&mut self, len: usize) {
        self.entries.truncate(len);
    }
}
