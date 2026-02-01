
pub fn initialize(engine: &ChainEngine, state_db: Arc<dyn DiskDB>, no_sta_dir: bool) {
      
    let _lk = engine.syncing.lock().unwrap();

    // store
    let store = engine.store.as_ref();
    let status = store.status();
    // check rebuild all
    let is_rebuild_all = no_sta_dir && (
        status.last_height.uint() > engine.cnf.unstable_block * 2
    );
    // build roller
    if is_rebuild_all {
        rebuild_all_blocks(engine);
    }else{        
        dev_count_switch_print(engine.cnf.dev_count_switch, state_db.as_ref());
        rebuild_unstable_blocks(engine);
    }
}

fn load_root_block(minter: &dyn Minter, store: &dyn Store) -> Arc<dyn Block> {
    let status = store.status();
    let rhein = status.root_height.uint();
    if rhein == 0 {
        return minter.genesis_block();
    }
    let Some((_, _, resblk)) = block::load_block_by_height(store, &BlockHeight::from(rhein)) else {
        panic!("cannot load root block {}", rhein)
    };
    resblk.into()
}

fn rebuild_unstable_blocks(engine: &ChainEngine) {
    // let finish_height = engine.store.status().last_height.uint();
    let mut roller = engine.tree.write().unwrap();
    let mut next_height = roller.root.height + 1;
    // rebuild unstable blocks
    print!("[Engine] Data: {}, rebuild ({})", engine.cnf.data_dir, next_height);
    loop {
        let Some((_hx, blkdata, block)) = block::load_block_by_height(engine.store.as_ref(), &next_height.into()) else {
            break;
        };
        let mut pkg = BlkPkg::new(block, blkdata);
        pkg.set_origin(BlkOrigin::Rebuild);
        let ier = insert_by(engine, &mut roller, pkg);
        if let Err(e) = ier {
            panic!("[State Panic] rebuild block {} state error: {}", next_height, e);
        }
        roll_by(engine, ier.unwrap()).unwrap();
        flush!("➢{}", next_height);
        next_height += 1;
    }    
    println!(" ok.");
}




fn rebuild_all_blocks(engine: &ChainEngine) {
    let finish_height = engine.store.status().last_height.uint();
    {   // updata root to height 0
        let mut temp = engine.tree.write().unwrap();
        let set_chunk = Arc::new((*temp).root.clone().update_to(engine.minter.genesis_block()));
        (*temp).root = set_chunk.clone();
        (*temp).head = set_chunk;
    }
    println!("[Database] scan all {} blocks to upgrade state db version, plase waiting...", finish_height);
    const STUFFCAP: usize = 20*1000*1000; // 20 mb

    std::thread::scope(|s| {
        let chsize = engine.cnf.unstable_block as usize;
        let (blkdtch, blkdtcv) = std::sync::mpsc::sync_channel(chsize);
        // read block
        s.spawn(move || {
            let mut block_datas = Vec::with_capacity(STUFFCAP); 
            let mut block_num: usize = 0; 
            for next_height in 1 .. {
                let Some(mut blkdata) = block::load_block_data_by_height(engine.store.as_ref(), &next_height.into()) else {
                    blkdtch.send(Some(Arc::new(block_datas))).unwrap();
                    blkdtch.send(None).unwrap(); // mark finish
                    break
                };
                block_datas.append(&mut blkdata);
                block_num += 1;
                // 10 mb or 10000 blocks
                if block_num >= 10000 || block_datas.len() >= STUFFCAP {
                    blkdtch.send(Some(Arc::new(block_datas))).unwrap();
                    block_datas = Vec::with_capacity(STUFFCAP);
                    block_num = 0;
                    // print process
                    let per = next_height as f32 / finish_height as f32;
                    flush!("\r{:10} ({:.2}%)", next_height, per * 100.0);
                }
                // next block
            }
            flush!("\r{:10} ({:.2}%) ", finish_height, 100.0);
        });

        // sync block
        s.spawn(move || {
            loop {
                let res = blkdtcv.recv();
                // println!("recv: {:?}", res);
                let Some(blkdatas) = res.unwrap() else {
                    break // finish
                };
                if blkdatas.as_ref().len() == 0 {
                    break
                }
                if let Err(e) = synchronize(engine, blkdatas, BlkOrigin::Rebuild) {
                    panic!("{}", e)
                }
            }
        });

    });

    print!("finish.\n");

}
