


pub struct InsertResult {    
    /*
    Keep the previous root alive while roll_by is pending.
    
    Why this is necessary:
    - Chunk::parent is a Weak pointer, so the new root does NOT strongly own its ancestors.
    - StateInst also uses a Weak parent chain.
    - During bulk sync, insert_by and roll_by are pipelined via channels; root may advance
      multiple times before the corresponding root state is committed.
    - If the old root is dropped early, the Weak parent chain can break and subsequent
      block execution may fall back to disk reads that are not yet populated, causing
      incorrect state.
    By sending the old root through the same channel as the InsertResult, we keep it
    alive exactly until roll_by consumes and commits the new root.
    */
    old_root_hold: Option<Arc<Chunk>>,
    pub old_root_height: u64,
    pub root_change: Option<Arc<Chunk>>,
    pub head_change: Option<Arc<Chunk>>,
    pub hash: Hash,
    pub block: BlkPkg,
}

pub fn insert_by(eng: &ChainEngine, tree: &mut Roller, mut blk: BlkPkg) -> Ret<InsertResult> {
    let orgi = blk.orgi;
    let fast_sync = (eng.cnf.fast_sync && orgi == BlkOrigin::Sync) || orgi == BlkOrigin::Rebuild;

    let height = blk.hein;
    let hash = blk.hash.clone();

    let old_root_height = tree.root.height;
    if height <= old_root_height || height > tree.head.height + 1 {
        return errf!("insert height must between [{}, {}] but got {}", old_root_height + 1, tree.head.height + 1, height);
    }

    let prev_hash = blk.objc.prevhash();
    let parent = tree.quick_find(prev_hash).ok_or(format!("not find prev block <{}, {}>", height - 1, prev_hash))?;
    if parent.height + 1 != height {
        return errf!("not find prev block <{}, {}>", height - 1, prev_hash);
    }

    if !fast_sync {
        if parent.childs.read().unwrap().iter().any(|c| c.hash == hash) {
            return errf!("repetitive block <{}, {}>", height, hash);
        }
        let parent_blk = parent.block.as_read();
        eng.minter.blk_verify(blk.objc.as_read(), parent_blk, eng.store.as_ref())?;
        block_verify(&eng.cnf, blk.objc.as_read(), blk.data().len(), parent_blk)?;
    }

    let prev_state = parent.state.clone();
    let mut sub_state = prev_state.fork_sub(Arc::downgrade(&prev_state));

    if height == 1 {
        eng.minter.initialize(sub_state.as_mut())?;
    }

    let chain_info = ChainInfo {
        fast_sync,
        diamond_form: eng.cnf.diamond_form,
        id: eng.cnf.chain_id,
    };

    let logs = Box::new(eng.logs.next(maybe!(is_open_vmlog(eng, height), height, 0)));
    let (new_state, new_logs) = blk.objc.execute(chain_info, sub_state, logs)?;

    if !fast_sync {
        blk.set_origin(orgi);
        eng.minter.blk_insert(&blk, new_state.as_ref(), prev_state.as_ref().as_ref())?;
    }

    // Snapshot current root. If root advances, we must keep this Arc alive until roll_by.
    let prev_root = tree.root.clone();

    let item = Arc::new(Chunk::new(blk.objc.clone(), Arc::new(new_state), new_logs.into(), Some(&parent)));
    let (root_change, head_change) = tree.insert(&parent, item);

    // Only carry old root when root actually advances.
    let old_root_hold = maybe!(root_change.is_some(), Some(prev_root), None);
    Ok(InsertResult { old_root_hold, old_root_height, root_change, head_change, hash, block: blk })
}


pub fn roll_by(eng: &ChainEngine, rid: InsertResult) -> Rerr {
    let InsertResult { old_root_hold, old_root_height, root_change, head_change, hash, block } = rid;
    let mut batch = MemKV::new();
    let is_sync     = block.orgi == BlkOrigin::Sync;
    let not_rebuild = block.orgi != BlkOrigin::Rebuild;
    if not_rebuild { // put block datas
        batch.put(hash.to_vec(), block.copy_data());
    }
    // if change root
    if let Some(new_root) = &root_change {
        // Commit state/logs only after store batch is durable.
        // This avoids: state advanced but CSK/block data not persisted (crash between insert_by and roll_by).
        new_root.state.write_to_disk();
        if is_open_vmlog(eng, new_root.logs.height()) {
            new_root.logs.write_to_disk();
        }
        eng.scaner.roll(new_root.block.clone(), new_root.state.clone(), eng.disk.clone());
        // Keep the old root alive until after state/logs are committed.
        // See InsertResult::old_root_hold comment for the rationale.
        let _old_root_hold = old_root_hold;
    }
    // if change head
    if let Some(new_head) = &head_change {
        let real_root_hei: u64 = match &root_change {
            Some(rt) => rt.height,
            _ => old_root_height,
        };
        if not_rebuild {
            batch.put(BlockStore::CSK.to_vec(), ChainStatus{
                root_height: BlockHeight::from(real_root_hei),
                last_height: BlockHeight::from(new_head.height),
            }.serialize());
            let mut skchk = new_head.clone();
            let mut skhei = BlockHeight::from(skchk.height);
            if is_sync {
                batch.put(skhei.to_vec(), skchk.hash.to_vec());
            } else {
                for _ in 0..eng.cnf.unstable_block + 1 {
                    batch.put(skhei.to_vec(), skchk.hash.to_vec());
                    skchk = match skchk.parent.upgrade() {
                        Some(h) => h,
                        _ => break,
                    };
                    skhei -= 1;
                }
            }
        }
    }
    // println!("roll_by eng.store.save_batch = {}", batch.len());
    if not_rebuild {
        eng.store.save_batch(&batch);
    }
    Ok(())
}

pub fn record_recent(eng: &ChainEngine, block: &dyn BlockRead) {
    let chei = block.height().uint() as i128;
    let deln = (eng.cnf.unstable_block * 2) as i128; // retain unstable * 2
    let deln = chei - deln;
    let mut rcts = eng.recent_blocks.lock().unwrap();
    rcts.retain(|x| x.height as i128 > deln);
    rcts.push_front(Arc::new(create_recent_block_info(block)));
}

pub fn record_avgfee(eng: &ChainEngine, block: &dyn BlockRead) {
    let mut rfees = eng.avgfees.lock().unwrap();
    let mut avgf = eng.cnf.lowest_fee_purity;
    let txs = block.transactions();
    let txnum = txs.len();
    if txnum >= 30 {
        let nmspx = txnum / 3;
        let mut allpry = 0u64;
        for i in nmspx .. nmspx * 2 {
            allpry += txs[i].fee_purity();
        }
        avgf = allpry / nmspx as u64;
    }
    rfees.push_front(avgf);
    if rfees.len() > 8 {
        rfees.pop_back();
    }
}


fn is_open_vmlog(eng: &ChainEngine, ck_hei: u64) -> bool {
    eng.cnf.vmlogs_enable && ck_hei >= eng.cnf.vmlogs_open_height
}
