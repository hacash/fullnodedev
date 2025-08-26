

pub trait State : Send + Sync {
    fn fork_sub(&self, _: Weak<dyn State>) -> Box<dyn State> { never!() }
    fn merge_sub(&mut self, _: Box<dyn State>) { never!() }
    // fn to_mem(&self) -> MemMap { never!() }

    // fn set_parent(&mut self, _: Arc<dyn State>) { never!() }
    fn disk(&self) -> Arc<dyn DiskDB> { never!() }
    fn write_to_disk(&self) { never!() }

    fn get(&self,     _: Vec<u8>) -> Option<Vec<u8>> { never!() }
    fn set(&mut self, _: Vec<u8>, _: Vec<u8>) { never!() }
    fn del(&mut self, _: Vec<u8>) { never!() }
}



pub trait Store : Send + Sync {





}








