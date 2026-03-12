

pub fn block_verify(cnf: &EngineConf, isrt_blk: &dyn BlockRead, blk_data_len: usize, prev_blk: &dyn BlockRead) -> Rerr {
        
    // check prev hash
    let prev_hx = isrt_blk.prevhash();
    let base_hx = prev_blk.hash();
    if *prev_hx != base_hx {
        return errf!("expected prev hash {} but got {}", base_hx, prev_hx)
    };
    // check time
    let prev_blk_time = prev_blk.timestamp().uint();
    let blk_time = isrt_blk.timestamp().uint();
    let cur_time = curtimes();
    if blk_time > cur_time {
        return errf!("block timestamp {} cannot exceed system timestamp {}", blk_time, cur_time)
    }
    // In debug mode, allow same timestamp for faster testing
    #[cfg(debug_assertions)]
    let time_check = blk_time < prev_blk_time;
    #[cfg(not(debug_assertions))]
    let time_check = blk_time <= prev_blk_time;
    if time_check {
        return errf!("block timestamp {} cannot be less than prev block timestamp {}", blk_time, prev_blk_time)
    }
    // check size
    if blk_data_len > cnf.max_block_size + 100 { // may 1MB + headmeta size
        return errf!("block size cannot exceed {} bytes", cnf.max_block_size + 100)
    }
    // check tx count
    let is_hash_with_fee = true;
    let txhxs = isrt_blk.transaction_hash_list(is_hash_with_fee); // hash with fee
    let txcount = isrt_blk.transaction_count().uint() as usize;
    if txcount < 1 {
        return err!("block txs cannot be empty; coinbase tx required")
    }
    if txcount > cnf.max_block_txs { // may 1000
        return errf!("block txs cannot exceed {}", cnf.max_block_txs)
    }
    if txcount != txhxs.len() {
        return errf!("block tx count expected {} but got {}", txhxs.len(), txcount)
    }
    // check tx total size and count
    let alltxs = isrt_blk.transactions();
    let mut txttsize = 0usize;
    let mut txttnum = 0usize;
    const CBTY: u8 =  TransactionCoinbase::TYPE;
    for tx in alltxs {
        let txty = tx.ty();
        // check only one coinbase at first
        if txttnum == 0 && txty != CBTY { // coinbase check
            return errf!("tx({}) type must be coinbase", txttnum)
        }
        if txttnum >= 1 && txty == CBTY { // must not be coinbase
            return errf!("tx({}) type cannot be coinbase", txttnum)  
        }
        let txsz = tx.size();
        if txsz > cnf.max_tx_size {
            return errf!("tx size cannot exceed {} bytes", cnf.max_tx_size);
        }
        // size count
        txttnum += 1;
        txttsize += txsz;
        if txty == CBTY {
            continue // igonre coinbase other check
        }
        let an = tx.action_count();
        if an != tx.actions().len() {
            return errf!("tx action count does not match")
        }
        if an > cnf.max_tx_actions {
            return errf!("tx action count cannot exceed {}", cnf.max_tx_actions);
        }
        // check time
        if tx.timestamp().uint() > cur_time {
            return errf!("tx timestamp {} cannot exceed now {}", tx.timestamp(), cur_time)
        }
        // Signature verification is enforced in tx.execute() (except fast_sync mode),
        // keeping execution as the single consensus anchor point.
    }
    // check size
    if txttnum != txcount {
        return errf!("block tx count expected {} but got {}", txcount, txttnum)        
    }
    if txttsize > cnf.max_block_size { // may 1MB
        return errf!("block txs total size cannot exceed {} bytes", cnf.max_block_size)
    }
    // check mrkl root
    let mkroot = block::calculate_mrklroot(&txhxs);
    let mrklrt = isrt_blk.mrklroot();
    if *mrklrt != mkroot {
        return errf!("block mrkl root expected {} but got {}", mkroot, mrklrt)
    }
    // ok 
    Ok(())
}
