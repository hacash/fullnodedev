

fn synchronize(this: &ChainEngine, mut datas: Vec<u8>) -> Rerr {
    let _isrtlock = inserting_lock(this, ISRT_STAT_SYNCING,
        "the blockchain is syncing and need wait"
    )?;

    let mut temp = this.tree.write().unwrap();
    let tree = temp.deref_mut();
    let latest_hei = tree.head.height;
    let hei_up = latest_hei + 1;
    let ubh = this.cnf.unstable_block;
    let hei_lo = if latest_hei > ubh { latest_hei - ubh + 1 } else { 1 };

    let Ok(blkhei) = BlockHeadOnlyHeight::build(&datas) else {
        return sync_warning("block data format error".to_string())
    };
    let insert_start_hei = blkhei.height.uint();
    if insert_start_hei < hei_lo || insert_start_hei > hei_up {
        return sync_warning(format!("height need between {} and {} but got {}", hei_lo, hei_up, insert_start_hei))
    }

    let tree_isr = hei_up - insert_start_hei;
    for _ in 0..tree_isr  {
        if datas.len() == 0 {
            break;
        }
        let seek;
        let blkobj = match block::block_create(&datas) {
            Ok((b, s)) => { seek = s; b },
            Err(e) => return errf!("block parse error: {}", e),
        };
        let right = datas.split_off(seek);
        let pkg = BlkPkg::new(blkobj, datas);
        let rid = insert_by(this, tree, pkg)?;
        roll_by(this, rid)?;
        datas = right;
    }

    if datas.len() == 0 {
        return Ok(())
    }

    // error channel
    let (errch, errcv) = std::sync::mpsc::channel();
    let errch1 = errch.clone();
    let errch2 = errch.clone();
    // data channel
    let (blkch, blkcv) = std::sync::mpsc::sync_channel(20);
    let (ridch, ridcv) = std::sync::mpsc::sync_channel(8);

    let mut need_blk_hei = tree.head.height + 1;
    let mut blockdts = datas.as_mut_slice();

    std::thread::scope(|s| {
        // parse block
        s.spawn(move || {
            loop {
                if blockdts.len() == 0 { break }
                let seek;
                let blkobj = match block::block_create(&blockdts) {
                    Ok((b, s)) => { seek = s; b },
                    Err(e) => {
                        errch.send(format!("block parse error: {}", e)).unwrap();
                        break;
                    }
                };
                let (left, right) = blockdts.split_at_mut(seek);
                let mut pkg = BlkPkg::new(blkobj, left.into());
                pkg.set_origin(BlkOrigin::Sync);
                if pkg.hein != need_blk_hei {
                    let _ = errch.send(format!("need block height {} but got {}", need_blk_hei, pkg.hein));
                    break;
                }
                if let Err(..) = blkch.send(pkg) { break }
                blockdts = right;
                need_blk_hei += 1;
            }
        });
        // do insert
        s.spawn(move || {
            loop {
                let Ok(blk) = blkcv.recv() else { break };
                let hei = blk.hein;
                let rid = match insert_by(this, tree, blk) {
                    Err(e) => {
                        let _ = errch1.send(format!("insert {} error: {}", hei, e));
                        break
                    },
                    Ok(r) => r,
                };
                if let Err(..) = ridch.send(rid) { break }
            }
        });
        // do roll
        loop {
            let Ok(rid) = ridcv.recv() else { break };
            if let Err(e) = roll_by(this, rid) {
                let _ = errch2.send(format!("do roll error: {}", e));
                break
            }
        }
        let _ = errch2.send("".to_string());
    });

    let e: String = errcv.recv().unwrap();
    if e.len() > 0 {
        return sync_warning(e)
    }
    Ok(())
}
