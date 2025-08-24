use crate::state::BlockStore;


    
pub fn load_block(store: &BlockStore, hx: &Hash) -> Option<(Vec<u8>, Box<dyn Block>)> {
	let Some(data) = store.block_data(&hx) else {
		return None
	};
	// parse
	match block::create(&data).map(|(b,_)|b) {
		Err(..) => None,
		Ok(b) => Some((data, b))
	}
}
    


pub fn load_block_by_height(store: &BlockStore, hei: &BlockHeight) -> Option<(Hash, Vec<u8>, Box<dyn Block>)> {
	let Some(hx) = store.block_hash(hei) else {
		return None
	};
	let Some((data, block)) = load_block(store, &hx) else {
		return None
	};
	Some((hx, data, block))
}



// BlockPkg
#[derive(Clone)]
pub struct BlockPkg {
	pub hein: u64,
	pub hash: Hash,
	pub data: Vec<u8>,
    pub objc: Box<dyn Block>,
    pub orgi: BlkOrigin,
}

impl BlockPkg {

	pub fn new(objc: Box<dyn Block>, data: Vec<u8>) -> Self {
		Self {
			orgi: BlkOrigin::UNKNOWN,
            hein: objc.height().uint(),
			hash: objc.hash(),
			data,
			objc,
		}
	}

	pub fn create(objc: Box<dyn Block>) -> Self {
        let data = objc.serialize();
		Self {
			orgi: BlkOrigin::UNKNOWN,
            hein: objc.height().uint(),
			hash: objc.hash(),
			data,
			objc,
		}
	}

	pub fn build(data: Vec<u8>) -> Ret<Self> {
		let (objc, _) = block::create(&data)?;
		Ok(Self {
			orgi: BlkOrigin::UNKNOWN,
            hein: objc.height().uint(),
			hash: objc.hash(),
			data,
			objc,
		})
	}

	pub fn into_block(self) -> Box<dyn Block> {
		self.objc
	}

	pub fn apart(self) -> (Hash, Box<dyn Block>, Vec<u8>) {
		(self.hash, self.objc, self.data)
	}

	pub fn set_origin(&mut self, orgi: BlkOrigin) {
		self.orgi = orgi;
	}


}



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
    let coinbase = &blk.transactions()[0];
    RecentBlockInfo {
        height:  blk.height().uint(),
        hash:    blk.hash(),
        prev:    blk.prevhash().clone(),
        txs:     blk.transaction_count().uint(), // transaction_count
        miner:   coinbase.main(),
        message: coinbase.message().to_readable_left(),
        reward:  coinbase.reward().clone(),
        time:    blk.timestamp().uint(),
        arrive:  curtimes(),
    }
}

