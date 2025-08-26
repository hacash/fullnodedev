

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

    fn status(&self) -> ChainStatus;
    fn save_block_data(&self, hx: &Hash, data: &Vec<u8>);
    fn save_block_hash(&self, hei: &BlockHeight, hx: &Hash);
    fn save_block_hash_path(&self, paths: Box<dyn Any>);
    fn save_batch(&self, batch: Box<dyn Any>);
    fn block_data(&self, hx: &Hash) -> Option<Vec<u8>>;
    fn block_hash(&self, hei: &BlockHeight) -> Option<Hash>;
    fn block_data_by_height(&self, hei: &BlockHeight) -> Option<(Hash, Vec<u8>)>;

}








