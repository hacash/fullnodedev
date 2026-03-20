


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



fn impl_blk_found(
    this: &HacashMinter,
    curblkhead: &dyn BlockRead,
    body: &Vec<u8>,
    sto: &dyn Store,
) -> RetBlkFound {
    let curhei = curblkhead.height().uint();
    let curdifnum = curblkhead.difficulty().uint();
    let blkspan = this.cnf.difficulty_adjust_blocks;
    let is_low_bid_child = {
        let biddings = this.bidding_prove.lock().unwrap();
        biddings.matches_low_bid_tip(curblkhead.prevhash())
    };
    if is_low_bid_child {
        let min_pow = current_low_bid_min_pow_hash(this, sto);
        if hash_bigger_than(curblkhead.hash().as_ref(), &min_pow) {
            return RetBlkFound::Reject
        }
        let Ok(mut blkp) = build_block_package(body.clone()) else {
            return RetBlkFound::Reject
        };
        blkp.set_origin(BlkOrigin::Discover);
        let mut biddings = this.bidding_prove.lock().unwrap();
        return maybe!(
            biddings.cache_low_bid_child(blkp),
            RetBlkFound::PendingCached,
            RetBlkFound::Reject
        )
    }

    if curhei <= blkspan {
        return RetBlkFound::Normal
    }
    if curhei < blkspan*200 && this.cnf.is_mainnet() {
        return RetBlkFound::Normal
    }

    let cblkhx = curblkhead.hash();
    if curhei % blkspan == 0 {
        let (_, difnum, _) = this.difficulty.req_cycle_block(curhei - 1, sto);
        let bign = u32_to_biguint(difnum).mul(4usize); // max is 4 times
        let mindiffhx = biguint_to_hash(&bign);
        if hash_bigger_than(cblkhx.as_ref(), &mindiffhx) {
            return RetBlkFound::Reject
        }
        return RetBlkFound::Normal
    }
    let (_, difnum, diffhx) = this.difficulty.req_cycle_block(curhei, sto);
    if difnum != curdifnum {
        return RetBlkFound::Reject
    }
    if hash_bigger_than(cblkhx.as_ref(), &diffhx) {
        return RetBlkFound::Reject
    }
    RetBlkFound::Normal
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
        if let Some(rhbf) = bidrecord.highest(curhei, dianum, prevsta, block.timestamp().uint()) {
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
                        bidrecord.add_low_bid_root(dianum, curblk.clone(), bidfee.clone());
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
        }
        bidrecord.remove_tx(dianum, txp.hash());
        bidrecord.roll(dianum);
    }
    Ok(())
}

fn current_low_bid_min_pow_hash(this: &HacashMinter, sto: &dyn Store) -> [u8; 32] {
    let latest = sto.status().last_height.uint();
    let diffnum = if latest == 0 {
        this.genesis_block.difficulty().uint()
    } else if let Some((_, blkdts)) = sto.block_data_by_height(&BlockHeight::from(latest)) {
        BlockIntro::must(&blkdts).difficulty().uint()
    } else {
        this.genesis_block.difficulty().uint()
    };
    let max_hash = u32_to_biguint(diffnum).mul(2usize);
    biguint_to_hash(&max_hash)
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
