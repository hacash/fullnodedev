fn impl_blk_found(
    this: &HacashMinter,
    curblkhead: &dyn BlockRead,
    body: &Vec<u8>,
    _: &dyn Store,
) -> Option<RetBlkFound> {
    let curhei = curblkhead.height().uint();
    let cblkhx = curblkhead.hash();
    let mut biddings = this.bidding_prove.lock().unwrap();
    let Some(min_pow) = biddings.min_pow_hash_by_prev(curblkhead.prevhash()) else {
        return None;
    };
    if hash_bigger_than(curblkhead.hash().as_ref(), &min_pow) {
        return Some(RetBlkFound::Reject)
    }
    let Ok(mut blkp) = build_block_package(body.clone()) else {
        return Some(RetBlkFound::Reject)
    };
    blkp.set_origin(BlkOrigin::Discover);
    if biddings.cache_low_bid_child(blkp) {
        biddings.mark_block_arrival(curhei, cblkhx);
        return Some(RetBlkFound::PendingCached)
    }
    Some(RetBlkFound::Reject)
}

fn impl_blk_arrive(
    this: &HacashMinter,
    curblkhead: &dyn BlockRead,
    _: &Vec<u8>,
    sto: &dyn Store,
) -> Rerr {
    let curhei = curblkhead.height().uint();
    let curdifnum = curblkhead.difficulty().uint();
    let cblkhx = curblkhead.hash();
    let blkspan = this.cnf.difficulty_adjust_blocks;

    if curhei <= blkspan {
        let mut biddings = this.bidding_prove.lock().unwrap();
        biddings.mark_block_arrival(curhei, cblkhx);
        return Ok(())
    }
    if curhei < blkspan*200 && this.cnf.is_mainnet() {
        let mut biddings = this.bidding_prove.lock().unwrap();
        biddings.mark_block_arrival(curhei, cblkhx);
        return Ok(())
    }

    if curhei % blkspan == 0 {
        let (_, difnum, _) = this.difficulty.req_cycle_block(curhei - 1, sto);
        let bign = u32_to_biguint(difnum).mul(4usize); // max is 4 times
        let mindiffhx = biguint_to_hash(&bign);
        if hash_bigger_than(cblkhx.as_ref(), &mindiffhx) {
            return errf!("block found height {} PoW hashrates check failed", curhei)
        }
        let mut biddings = this.bidding_prove.lock().unwrap();
        biddings.mark_block_arrival(curhei, cblkhx);
        return Ok(())
    }
    let (_, difnum, diffhx) = this.difficulty.req_cycle_block(curhei, sto);
    if difnum != curdifnum {
        return errf!("block found height {} PoW difficulty check failed: expected {} but got {}", curhei, difnum, curdifnum)
    }
    if hash_bigger_than(cblkhx.as_ref(), &diffhx) {
        return errf!("block found height {} PoW hashrates check failed", curhei)
    }
    let mut biddings = this.bidding_prove.lock().unwrap();
    biddings.mark_block_arrival(curhei, cblkhx);
    Ok(())
}
