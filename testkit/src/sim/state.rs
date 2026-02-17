use basis::component::MemMap;
use basis::interface::State;

#[derive(Default, Clone)]
pub struct ForkableMemState {
    parent: std::sync::Weak<Box<dyn State>>,
    mem: MemMap,
}

impl State for ForkableMemState {
    fn fork_sub(&self, p: std::sync::Weak<Box<dyn State>>) -> Box<dyn State> {
        Box::new(Self {
            parent: p,
            mem: MemMap::default(),
        })
    }

    fn merge_sub(&mut self, sta: Box<dyn State>) {
        self.mem.extend(sta.as_mem().clone());
    }

    fn detach(&mut self) {
        self.parent = std::sync::Weak::<Box<dyn State>>::new();
    }

    fn clone_state(&self) -> Box<dyn State> {
        Box::new(self.clone())
    }

    fn as_mem(&self) -> &MemMap {
        &self.mem
    }

    fn get(&self, k: Vec<u8>) -> Option<Vec<u8>> {
        if let Some(v) = self.mem.get(&k) {
            return v.clone();
        }
        if let Some(parent) = self.parent.upgrade() {
            return parent.get(k);
        }
        None
    }

    fn set(&mut self, k: Vec<u8>, v: Vec<u8>) {
        self.mem.insert(k, Some(v));
    }

    fn del(&mut self, k: Vec<u8>) {
        self.mem.insert(k, None);
    }
}

#[derive(Default, Clone)]
pub struct FlatMemState {
    mem: MemMap,
}

impl State for FlatMemState {
    fn fork_sub(&self, _: std::sync::Weak<Box<dyn State>>) -> Box<dyn State> {
        Box::new(Self {
            mem: MemMap::default(),
        })
    }

    fn merge_sub(&mut self, sta: Box<dyn State>) {
        self.mem.extend(sta.as_mem().clone());
    }

    fn detach(&mut self) {}

    fn clone_state(&self) -> Box<dyn State> {
        Box::new(self.clone())
    }

    fn as_mem(&self) -> &MemMap {
        &self.mem
    }

    fn get(&self, k: Vec<u8>) -> Option<Vec<u8>> {
        self.mem.get(&k).and_then(|v| v.clone())
    }

    fn set(&mut self, k: Vec<u8>, v: Vec<u8>) {
        self.mem.insert(k, Some(v));
    }

    fn del(&mut self, k: Vec<u8>) {
        self.mem.insert(k, None);
    }
}
