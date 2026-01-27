
pub fn initialize(engine: &ChainEngine, state_db: Arc<dyn DiskDB>, state_exists: bool) {
    let is_state_upgrade = !state_exists;
    let root_block = load_root_block(engine.minter.as_ref(), engine.store.as_ref(), is_state_upgrade);
    let state = StateInst::build(state_db.clone(), None);
    
    dev_count_switch_print(engine.cnf.dev_count_switch, state_db.as_ref());
    
    let state: Arc<Box<dyn State>> = Arc::new(Box::new(state));

    {
        let mut tree = engine.tree.write().unwrap();
        *tree = Roller::new(root_block, state, engine.logs.clone(), engine.cnf.unstable_block);
    }

    rebuild_unstable_blocks(engine);
}

fn load_root_block(minter: &dyn Minter, store: &dyn Store, is_state_upgrade: bool) -> Arc<dyn Block> {
    if is_state_upgrade {
        return minter.genesis_block();
    }
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
    let status = engine.store.status();
    let mut tree = engine.tree.write().unwrap();
    let roller = tree.deref_mut();
    let mut next_height = roller.root.height;
    let finish_height = status.last_height.uint();
    let is_all_rebuild = finish_height.saturating_sub(next_height) > 20;

    if is_all_rebuild {
        println!("[Database] check all blocks to upgrade state db version, plase waiting...");
    } else {
        print!("[Engine] Data: {}, rebuild ({})", engine.cnf.data_dir, next_height);
    }

    loop {
        next_height += 1;
        let Some((_hx, blkdata, block)) = block::load_block_by_height(engine.store.as_ref(), &next_height.into()) else {
            break;
        };
        if is_all_rebuild {
            if next_height % 631 == 0 {
                let per = next_height as f32 / finish_height as f32;
                flush!("\r{:10} ({:.2}%)", next_height, per * 100.0);
            }
        } else {
            flush!("➢{}", next_height);
        }
        let pkg = BlkPkg::new(block, blkdata);
        let ier = insert_by(engine, roller, pkg);
        if let Err(e) = ier {
            panic!("[State Panic] rebuild block {} state error: {}", next_height, e);
        }
        roll_by(engine, ier.unwrap()).unwrap();
    }

    if is_all_rebuild {
        flush!("\r{:10} ({:.2}%)", next_height - 1, 100.0);
    } else {
        println!(" ok.");
    }
}
