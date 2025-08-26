
pub struct BlockStore {
    disk: Arc<dyn DiskDB>
}

impl BlockStore {

    pub const CSK: &[u8] = b"chain_status";

    
    pub fn wrap(disk: Arc<dyn DiskDB>) -> Self {
        Self { disk }
    }

    pub fn status(&self) -> ChainStatus {
        let mut stat = ChainStatus::default();
        match self.disk.read(Self::CSK) {
            None => stat,
            Some(v) => {
                stat.parse(&v).unwrap(); // must
                stat
            }
        }
    }
    
    pub fn save_block_data(&self, hx: &Hash, data: &Vec<u8>) {
        self.disk.save(hx.as_ref(), &data)
    }
    
    pub fn save_block_hash(&self, hei: &BlockHeight, hx: &Hash) {
        self.disk.save(&hei.to_bytes(), hx.as_ref())
    }
    
    // MemBatch
    pub fn save_block_hash_path(&self, paths: Box<dyn Any>) {
        self.disk.write_batch(paths)
    }
    
    // MemBatch
    pub fn save_batch(&self, batch: Box<dyn Any>) {
        self.disk.write_batch(batch)
    }

    // read
    
    pub fn block_data(&self, hx: &Hash) -> Option<Vec<u8>> {
        self.disk.read(hx.as_ref())
    }

    pub fn block_hash(&self, hei: &BlockHeight) -> Option<Hash> {
        let Some(hx) = self.disk.read(&hei.to_bytes()) else {
            return None
        };
        Some(Hash::must(&hx))
    }
    
    pub fn block_data_by_height(&self, hei: &BlockHeight) -> Option<(Hash, Vec<u8>)> {
        let Some(hx) = self.block_hash(hei) else {
            return None
        };
        self.block_data(&hx).map(|d|(hx, d))
    }

}