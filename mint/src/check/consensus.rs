


fn impl_tx_submit(this: &HacashMinter, engine: &dyn EngineRead, txp: &TxPkg) -> Rerr {
    let txr = txp.tx_read();
    let curr_hei = engine.latest_block().height().uint();
    let next_hei = curr_hei + 1;
    let Some(diamintact) = action::pickout_diamond_mint_action(txr) else {
        return Ok(()) // other normal tx
    };
    if next_hei % 5 == 0 {
        return errf!("diamond mint transaction cannot be submitted after height ending in 4 or 9")
    }
    check_diamond_mint_minimum_bidding_fee(next_hei, txr, &diamintact)?;
    let mut biddings = this.bidding_prove.lock().unwrap();
    biddings.record(curr_hei, txp, &diamintact);
    Ok(())
}



fn impl_blk_found_pending(
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

fn impl_blk_found(
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



fn impl_blk_verify(this: &HacashMinter, curblk: &dyn BlockRead, prevblk: &dyn BlockRead, sto: &dyn Store) -> Rerr {
    let curhei = curblk.height().uint(); // u64
    let smaxh = this.cnf.sync_maxh;
    if smaxh > 0 && curhei > smaxh {
        return errf!("config [mint].height_max limit: {}", smaxh)
    }
    // verify coinbase
    verify_coinbase(curhei, curblk.coinbase_transaction()?)?;
    // check difficulty
    let blkcln = this.cnf.difficulty_adjust_blocks; // 288
    if curhei < blkcln*200 && this.cnf.is_mainnet() {
        return Ok(()) // not check, compatible history code
    }
    // check
    let curn = curblk.difficulty().uint(); // u32
    let (tarn, tarhx, _tarbign) = this.next_difficulty(prevblk, sto);
    if tarn != curn {
        return errf!("height {} PoW difficulty check failed: expected {} but got {}", curhei, tarn, curn)
    }
    if hash_bigger_than(curblk.hash().as_ref(), &tarhx) {
        return errf!("height {} PoW hashrates check failed: must not exceed {} but got {}", 
            curhei, hex::encode(tarhx),  hex::encode(curblk.hash()))
    }
    Ok(())
}



fn impl_blk_insert(this: &HacashMinter, curblk: &BlkPkg, _sta: &dyn State, prev: &dyn State) -> Rerr {
    check_highest_bid_of_block(this, curblk, prev)?;
    Ok(())
}



/************************/



fn check_highest_bid_of_block(this: &HacashMinter, curblk: &BlkPkg, prevsta: &dyn State) -> Rerr {
    let curhei = curblk.hein();
    if curhei % 5 != 0 {
        return  Ok(())
    }
    let block = curblk.block_read();
    if let Some((tidx, txp, diamint)) = action::pickout_diamond_mint_action_from_block(block) {
        const CKN: u32 = DIAMOND_ABOVE_NUMBER_OF_MIN_FEE_AND_FORCE_CHECK_HIGHEST;
        if tidx != 1 && curhei > 600000 {
            return errf!("diamond mint transaction must be the first tx in block")
        }
        let dianum  = *diamint.d.number;
        let bidfee  = txp.fee().clone();
        check_diamond_mint_minimum_bidding_fee(curhei, txp.as_read(), &diamint)?;
        let mut bidrecord = this.bidding_prove.lock().unwrap();
        let t4blkt = bidrecord.prev_block_arrive_time(block.prevhash());
        let rhbf = bidrecord.highest(curhei, dianum, prevsta, t4blkt);
        if bidfee < rhbf {
            if dianum > CKN {
                if bidrecord.is_replay_allowed(&curblk.hash()) {
                    println!(
                        "[MintLowBid] replay low bid accepted height={} hash={} diamond={} fee={} fence={}",
                        curhei,
                        curblk.hash().half(),
                        dianum,
                        bidfee,
                        rhbf,
                    );
                } else {
                    if !bidrecord.add_low_bid_root(dianum, curblk.clone(), bidfee.clone()) {
                        return errf!("{}", LOW_BID_CACHE_FULL_ERR);
                    }
                    println!(
                        "[MintLowBid] low root detected height={} hash={} diamond={} fee={} fence={}",
                        curhei,
                        curblk.hash().half(),
                        dianum,
                        bidfee,
                        rhbf,
                    );
                    return errf!("{}", LOW_BID_PENDING_ERR)
                }
            }
        }
        bidrecord.remove_tx(dianum, txp.hash());
        bidrecord.roll(dianum);
    }
    Ok(())
}

fn check_diamond_mint_minimum_bidding_fee(next_hei: u64, tx: &dyn TransactionRead, dmact: &action::DiamondMint) -> Rerr {
    const CKN: u32 = DIAMOND_ABOVE_NUMBER_OF_MIN_FEE_AND_FORCE_CHECK_HIGHEST;
    let bidmin = genesis::block_reward(next_hei);
    let bidfee  = tx.fee().clone();
    let dianum  = *dmact.d.number;
    if bidfee < bidmin && dianum > CKN {
        return errf!("diamond bidding fee {} cannot be less than {} after number {}", bidfee, bidmin, CKN)
    }
    Ok(())
}
