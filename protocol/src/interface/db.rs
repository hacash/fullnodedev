

pub trait MemDB : Send + Sync {
    fn new() -> Self where Self: Sized { unimplemented!() }
    fn del(&mut self, _: Vec<u8>) { unimplemented!() }
    fn put(&mut self, _: Vec<u8>, _: Vec<u8>) { unimplemented!() }
    fn get(&self, _: &Vec<u8>) -> Option<Option<Vec<u8>>> { unimplemented!() }
    fn to_batch(&self) -> Box<dyn Any> { unimplemented!() }
}


pub trait MemBatch {
    fn new() -> Self where Self: Sized { unimplemented!() }
    fn del(&mut self, _: &[u8]) { unimplemented!() }
    fn put(&mut self, _: &[u8], _: &[u8]) { unimplemented!() }
}


pub trait DiskDB : Send + Sync {
    fn read(&self, _: &[u8]) -> Option<Vec<u8>>;
    fn save(&self, _: &[u8], _: &[u8] );
    fn drop(&self, _: &[u8]);
    fn write(&self, _: Box<dyn Any>); // dyn MemDB
    fn write_batch(&self, _: Box<dyn Any>); // dyn MemBatch
    // debug
    fn for_each(&self, _: &mut dyn FnMut(Vec<u8>, Vec<u8>)->bool) {}
}



