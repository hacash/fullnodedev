use basis::component::*;

#[derive(Default)]
pub struct StateMem {
    mem: MemKV
}

impl State for StateMem {
    fn get(&self, k: Vec<u8>) -> Option<Vec<u8>> {
        match self.mem.get(&k) {
            Some(v) => v.clone(),
            _ => None,
        }
    }
    fn set(&mut self, k: Vec<u8>, v: Vec<u8>) {
        self.mem.put(k, v)
    }
    fn del(&mut self, k: Vec<u8>) {
        self.mem.del(k) // add del mark
    }
}

