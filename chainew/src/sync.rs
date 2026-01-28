

fn synchronize(this: &ChainEngine, datas: Arc<Vec<u8>>, ori: BlkOrigin) -> Rerr {
    let _isrtlock = inserting_lock(this, ISRT_STAT_SYNCING,
        "the blockchain is syncing and need wait"
    )?;
    // let _lk = this.isrtlk.lock().unwrap();
    // let datas = &datas[..];
    let mut temp = this.tree.write().unwrap();
    let tree = temp.deref_mut();
    let hei_min = tree.root.height + 1;
    let hei_max = tree.head.height + 1;
    // let latest_hei = tree.head.height;
    // let hei_up = latest_hei + 1;
    // let ubh = this.cnf.unstable_block;
    // let hei_lo = if latest_hei > ubh { latest_hei - ubh + 1 } else { 1 };

    let Ok(blkhei) = BlockHeadOnlyHeight::build(datas.as_ref()) else {
        return sync_warning("block data format error".to_string())
    };
    let hei_start = blkhei.height.uint();
    if hei_start < hei_min || hei_start > hei_max {
        return sync_warning(format!("insert height need between {} and {} but got {}", hei_min, hei_max, hei_start))
    }

    // error channel
    let (errch, errcv) = std::sync::mpsc::channel();
    let errch1 = errch.clone();
    let errch2 = errch.clone();
    // data channel
    let chsize = this.cnf.unstable_block as usize * 2;
    let (blkch, blkcv) = std::sync::mpsc::sync_channel(chsize);
    let (ridch, ridcv) = std::sync::mpsc::sync_channel(chsize);

    let mut need_blk_hei = tree.head.height + 1;
    // let mut blockdts = datas.as_mut_slice();
    let sizecap = datas.as_ref().len();
    let mut seek = 0;

    std::thread::scope(|s| {
        // parse block
        s.spawn(move || {
            loop {
                if seek >= sizecap { break }
                let (blkobj, size) = match block::block_create(&datas.as_ref()[seek..]) {
                    Ok((b, s)) => (b, s),
                    Err(e) => {
                        errch.send(format!("block parse error: {}", e)).unwrap();
                        break;
                    }
                };
                // println!(" s.spawn block_create = {}", need_blk_hei);
                let mut pkg = BlkPkg::from(blkobj, datas.clone(), seek, size);
                seek += size;
                if pkg.hein != need_blk_hei {
                    let _ = errch.send(format!("need block height {} but got {}", need_blk_hei, pkg.hein));
                    break;
                }                
                pkg.set_origin(ori); // Sync or Rebuild
                if let Err(..) = blkch.send(pkg) { break }
                // println!(" s.spawn block_create = {} ok",need_blk_hei);
                need_blk_hei += 1;
            }
        });
        // do insert
        s.spawn(move || {
            loop {
                let Ok(blk) = blkcv.recv() else { break };
                let hei = blk.hein;
                // println!("insert_by = {}", hei);
                let rid = match insert_by(this, tree, blk) {
                    Err(e) => {
                        let _ = errch1.send(format!("insert {} error: {}", hei, e));
                        break
                    },
                    Ok(r) => r,
                };
                // println!("insert_by = {:?} ok", hei);
                if let Err(..) = ridch.send(rid) { break }
                // println!("insert_by = {} send ok", hei);
            }
        });
        // do roll
        loop {
            let Ok(rid) = ridcv.recv() else { break };
            // let hei = rid.block.hein;
            // println!("roll_by = root:{}, head:{}", 
                // rid.root_change.is_some(), rid.head_change.is_some());
            if let Err(e) = roll_by(this, rid) {
                let _ = errch2.send(format!("do roll error: {}", e));
                break
            }
            // println!("roll_by = {} ok", hei);
        }
        let _ = errch2.send("".to_string());
    });

    let e: String = errcv.recv().unwrap();
    if e.len() > 0 {
        return sync_warning(e)
    }
    Ok(())
}
