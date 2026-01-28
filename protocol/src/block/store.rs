
    
pub fn load_block(store: &dyn Store, hx: &Hash) -> Option<(Vec<u8>, Box<dyn Block>)> {
	let Some(data) = store.block_data(&hx) else {
		return None
	};
	// parse
	match block_create(&data).map(|(b,_)|b) {
		Err(..) => None,
		Ok(b) => Some((data, b))
	}
}
    


pub fn load_block_data_by_height(store: &dyn Store, hei: &BlockHeight) -> Option<Vec<u8>> {
	let Some(hx) = store.block_hash(hei) else {
		return None
	};	
	let Some(data) = store.block_data(&hx) else {
		return None
	};
	Some(data)
}

pub fn load_block_by_height(store: &dyn Store, hei: &BlockHeight) -> Option<(Hash, Vec<u8>, Box<dyn Block>)> {
	let Some(hx) = store.block_hash(hei) else {
		return None
	};
	let Some((data, block)) = load_block(store, &hx) else {
		return None
	};
	Some((hx, data, block))
}