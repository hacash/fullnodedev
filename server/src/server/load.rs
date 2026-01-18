
pub fn api_load_block(apictx: &ApiCtx, store: &dyn Store, key: &String) -> Ret<Arc<BlkPkg>> {
    api_load_block_from_cache(apictx, store, key, true)
}

    
// load block from cache or disk, key = height or hash
pub fn api_load_block_from_cache(apictx: &ApiCtx, store: &dyn Store, key: &String, with_cache: bool) -> Ret<Arc<BlkPkg>> {
    let mut hash = Hash::from([0u8; 32]);
    let mut height = BlockHeight::from(0);
    if key.len() == 64 {
        if let Ok(hx) = hex::decode(key) {
            hash = Hash::from(hx.try_into().unwrap());
        }
    }else{
        if let Ok(num) = key.parse::<u64>() {
            height = BlockHeight::from(num);
        }
    }
    // check cache
    if with_cache {
        let list = apictx.blocks.lock().unwrap();
        for blk in list.iter() {
            if height == *blk.objc.height() || hash == blk.hash {
                return Ok(blk.clone())
            }
        }
    }
    // read from disk
    let blkdts;
    if *height > 0 {
        blkdts = store.block_data_by_height(&height).map(|(_,a)|a);
    }else{
        blkdts = store.block_data(&hash);
    }
    if let None = blkdts {
        return errf!("block not find")
    }
    let Ok(blkpkg) = block::build_block_package(blkdts.unwrap()) else {
        return errf!("block parse error")
    };
    // ok
    let blkcp = Arc::new(blkpkg);
    if with_cache {
        let mut list = apictx.blocks.lock().unwrap();
        list.push_front(blkcp.clone());
        if list.len() > apictx.blocks_max {
            list.pop_back(); // cache limit 
        }
    }
    return Ok(blkcp)
}




