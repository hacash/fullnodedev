


#[derive(Copy, Clone, PartialEq)]
enum HeadChangeKind {
    None,
    Extend,
    Reorg,
}

struct InsertResult {
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
    old_root_hold: Option<ChunkRef>,
    old_root_height: u64,
    root_change: Option<ChunkRef>,
    head_change: Option<ChunkRef>,
    head_change_kind: HeadChangeKind,
    hash: Hash,
    block: BlkPkg,
}

struct InsertBlockIntroSource<'a> {
    store: &'a dyn Store,
    anchor: ChunkRef,
    root_height: u64,
}

impl<'a> InsertBlockIntroSource<'a> {
    fn new(store: &'a dyn Store, anchor: ChunkRef, root_height: u64) -> Self {
        Self { store, anchor, root_height }
    }
}

impl BlockIntroSource for InsertBlockIntroSource<'_> {
    fn block_intro(&self, hei: u64) -> Option<Box<dyn BlockRead>> {
        if hei > self.root_height {
            let blk = Roller::ancestor_at(&self.anchor, hei)?.block();
            return Some(Box::new(protocol::block::BlockIntro {
                head: protocol::block::BlockHead {
                    version: blk.version().clone(),
                    height: blk.height().clone(),
                    timestamp: blk.timestamp().clone(),
                    prevhash: blk.prevhash().clone(),
                    mrklroot: blk.mrklroot().clone(),
                    transaction_count: blk.transaction_count().clone(),
                },
                meta: protocol::block::BlockMeta {
                    nonce: blk.nonce().clone(),
                    difficulty: blk.difficulty().clone(),
                    witness_stage: Fixed2::default(),
                },
            }));
        }
        let (_, blkdts) = self.store.block_data_by_height(&BlockHeight::from(hei))?;
        protocol::block::BlockIntro::build(&blkdts).ok().map(|v| Box::new(v) as Box<dyn BlockRead>)
    }
}

fn insert_by(eng: &ChainEngine, tree: &mut Roller, mut blk: BlkPkg) -> Ret<InsertResult> {
    let orgi = blk.origin();
    let fast_sync = (eng.cnf.fast_sync && orgi == BlkOrigin::Sync) || orgi == BlkOrigin::Rebuild;

    let height = blk.hein();
    let hash = blk.hash();

    let old_root_height = tree.root_height();
    if height <= old_root_height || height > tree.head_height() + 1 {
        return errf!("insert height must be between [{}, {}] but got {}", old_root_height + 1, tree.head_height() + 1, height);
    }

    let prev_hash = blk.block().prevhash();
    let parent = tree.quick_find(prev_hash).ok_or(format!("prev block <{}, {}> not found", height - 1, prev_hash))?;
    if parent.height() + 1 != height {
        return errf!("prev block <{}, {}> not found", height - 1, prev_hash);
    }

    if !fast_sync {
        if tree.has_child_hash(&parent, &hash) {
            return errf!("block already exists");
        }
        let parent_block = parent.block();
        let parent_blk = parent_block.as_read();
        let src = InsertBlockIntroSource::new(eng.store.as_ref(), parent.clone(), old_root_height);
        // Stage 4: minter pre-exec block gate.
        eng.minter.blk_verify(blk.block_read(), parent_blk, &src)?;
        // Stage 5: generic structural block gate.
        block_verify(&eng.cnf, blk.block_read(), blk.data().len(), parent_blk)?;
    }

    let prev_state = parent.state();
    let sub_state = prev_state.fork_sub(Arc::downgrade(&prev_state));

    let chain_info = ChainInfo {
        fast_sync,
        diamond_form: eng.cnf.diamond_form,
        id: eng.cnf.chain_id,
    };

    let logs = Box::new(eng.logs.next(maybe!(is_open_vmlog(eng, height), height, 0)));
    let (new_state, new_logs) = blk.block().execute(chain_info, sub_state, logs)?;

    if !fast_sync {
        blk.set_origin(orgi);
        let new_state_ref: &dyn State = new_state.as_ref();
        let prev_state_ref: &dyn State = prev_state.as_ref().as_ref();
        // Stage 8: final gate after execution and before forktree insertion.
        eng.minter.blk_insert(&blk, new_state_ref, prev_state_ref)?;
    }

    // Snapshot current root. If root advances, we must keep this Arc alive until roll_by.
    let prev_root = tree.root();
    let prev_head = tree.head();
    let extend_old_head = parent.ptr_eq(&prev_head);

    let new_logs: Arc<dyn Logs> = Arc::from(new_logs);
    let (root_change, head_change) = tree.insert_child(
        &parent,
        blk.block_clone(),
        Arc::new(new_state),
        new_logs,
        fast_sync,
    )?;
    let head_change_kind = match &head_change {
        None => HeadChangeKind::None,
        Some(_) => maybe!(extend_old_head, HeadChangeKind::Extend, HeadChangeKind::Reorg),
    };

    // Only carry old root when root actually advances.
    let old_root_hold = maybe!(root_change.is_some(), Some(prev_root), None);
    Ok(InsertResult { old_root_hold, old_root_height, root_change, head_change, head_change_kind, hash, block: blk })
}


fn roll_by(eng: &ChainEngine, rid: InsertResult) -> Rerr {
    let InsertResult { old_root_hold, old_root_height, root_change, head_change, head_change_kind, hash, block } = rid;
    let mut batch = MemKV::new();
    let not_rebuild = block.origin() != BlkOrigin::Rebuild;
    if not_rebuild { // put block datas
        batch.put(hash.to_vec(), block.copy_data());
    }
    // if change root
    if let Some(new_root) = &root_change {
        // Persist state/logs before store batch commit.
        // If a crash happens before batch durability, restart from old store root can replay and reconcile.
        new_root.state().write_to_disk();
        if is_open_vmlog(eng, new_root.logs().height()) {
            new_root.logs().write_to_disk();
        }
        if not_rebuild {
            eng.scaner.roll(new_root.block(), new_root.state(), eng.disk.clone());
        }
        // Keep the old root alive until after state/logs are committed.
        // See InsertResult::old_root_hold comment for the rationale.
        let _old_root_hold = old_root_hold;
    }
    // if change head
    if let Some(new_head) = &head_change {
        let real_root_hei: u64 = match &root_change {
            Some(rt) => rt.height(),
            _ => old_root_height,
        };
        if not_rebuild {
            batch.put(BlockStore::CSK.to_vec(), ChainStatus{
                root_height: BlockHeight::from(real_root_hei),
                last_height: BlockHeight::from(new_head.height()),
            }.serialize());
            if head_change_kind == HeadChangeKind::Reorg {
                let reorg_depth = new_head.height()
                    .checked_sub(real_root_hei)
                    .and_then(|v| v.checked_add(1))
                    .ok_or(format!("invalid reorg depth: head {} root {}", new_head.height(), real_root_hei))?;
                for (hei, hx) in Roller::collect_back_hashes(new_head, reorg_depth) {
                    batch.put(hei.to_vec(), hx.to_vec());
                }
            } else {
                let skhei = BlockHeight::from(new_head.height());
                batch.put(skhei.to_vec(), new_head.hash().to_vec());
            }
        }
    }
    // println!("roll_by eng.store.save_batch = {}", batch.len());
    if not_rebuild {
        eng.store.save_batch(&batch);
    }
    Ok(())
}

fn record_recent(eng: &ChainEngine, block: &dyn BlockRead, root_height: u64) {
    let deln = root_height.saturating_sub(eng.cnf.unstable_block);
    let mut rcts = eng.recent_blocks.lock().unwrap();
    rcts.retain(|x| x.height > deln);
    rcts.push_front(Arc::new(create_recent_block_info(block)));
}

fn record_avgfee(eng: &ChainEngine, block: &dyn BlockRead) {
    let mut rfees = eng.avgfees.lock().unwrap();
    let mut avgf = eng.cnf.lowest_fee_purity;
    let txs = block.transactions();
    let txnum = txs.len();
    if txnum >= 30 {
        let nmspx = txnum / 3;
        let mut allpry = 0u128;
        for i in nmspx .. nmspx * 2 {
            allpry += txs[i].fee_purity() as u128;
        }
        avgf = (allpry / nmspx as u128) as u64;
    }
    rfees.push_front(avgf);
    if rfees.len() > 8 {
        rfees.pop_back();
    }
}


fn is_open_vmlog(eng: &ChainEngine, ck_hei: u64) -> bool {
    eng.cnf.vm_log_enable && ck_hei >= eng.cnf.vm_log_open_height
}
