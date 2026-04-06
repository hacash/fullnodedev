
#[derive(Debug, Clone)]
pub struct BlkPkg {
    data: Arc<Vec<u8>>,
    seek: usize,
    size: usize,
    orgi: BlkOrigin,
    objc: Arc<dyn Block>,
    hash: Hash,
    hein: u64, // block height
}


impl_pkg_common!{ BlkPkg, Block, BlkOrigin }

impl BlkPkg {

    pub fn from(objc: Box<dyn Block>, data: Arc<Vec<u8>>, seek: usize, size: usize) -> Self {
        Self {
            orgi: BlkOrigin::Unknown,
            hein: objc.height().uint(),
            hash: objc.hash(),
            data,
            seek,
            size,
            objc: objc.into(),
        }
    }

    pub fn new(objc: Box<dyn Block>, data: Vec<u8>) -> Self {
        let size = data.len();
        Self::from(objc, Arc::new(data), 0, size)
    }

    pub fn block(&self) -> &dyn Block {
        self.objc.as_ref()
    }

    pub fn block_read(&self) -> &dyn BlockRead {
        self.objc.as_read()
    }

    pub fn block_clone(&self) -> Arc<dyn Block> {
        self.objc.clone()
    }

    pub fn hash(&self) -> Hash {
        self.hash
    }

    pub fn hein(&self) -> u64 {
        self.hein
    }

}


/***********************************************************/





pub struct RecentBlockInfo { 
    pub height:  u64,
    pub hash:    Hash,
    pub prev:    Hash,
    pub txs:     u32, /* transaction_count */
    pub miner:   Address,
    pub message: String,
    pub reward:  Amount,
    pub time:    u64,
    pub arrive:  u64,
}


pub fn create_recent_block_info(blk: &dyn BlockRead) -> RecentBlockInfo {
    let ptx = blk.prelude_transaction().expect("block must have prelude");
    let miner = ptx.author().unwrap_or_default();
    let message = ptx
        .block_message()
        .map(|msg| msg.to_readable_left())
        .unwrap_or_else(|| s!(""));
    let reward = ptx.block_reward().cloned().unwrap_or_else(Amount::zero);
    RecentBlockInfo {
        height:  blk.height().uint(),
        hash:    blk.hash(),
        prev:    blk.prevhash().clone(),
        txs:     blk.transaction_count().uint(), // transaction_count
        miner,
        message,
        reward,
        time:    blk.timestamp().uint(),
        arrive:  curtimes(),
    }
}
