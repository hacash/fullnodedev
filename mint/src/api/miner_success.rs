fn miner_success(ctx: &ApiExecCtx, req: ApiRequest) -> ApiResponse {
    if !ctx.engine.config().miner_enable {
        return api_error("miner not enable");
    }

    let height = req.query_u64("height", 0);
    let block_nonce = q_u32(&req, "block_nonce", 0);
    let coinbase_nonce = q_string(&req, "coinbase_nonce", "");

    let mut success_stuff = {
        let mut stf = MINER_PENDING_BLOCK.lock().unwrap();
        if stf.is_empty() {
            return api_error("pending block not yet");
        }
        let mut found_idx = None;
        for i in 0..stf.len() {
            if *stf[i].height == height {
                found_idx = Some(i);
                break;
            }
        }
        let Some(stfidx) = found_idx else {
            return api_error(&format!("pending block height {} not find", height));
        };
        let tarstf = &mut stf[stfidx];

        let Ok(cb_nonce_bytes) = hex::decode(coinbase_nonce.as_bytes()) else {
            return api_error("coinbase nonce format error");
        };
        if cb_nonce_bytes.len() != Hash::SIZE {
            return api_error("coinbase nonce length error");
        }

        tarstf.block.set_nonce(Uint4::from(block_nonce));
        tarstf
            .coinbase_tx
            .set_nonce(Hash::from(cb_nonce_bytes.try_into().unwrap()));
        let cbhx = tarstf.coinbase_tx.hash();
        let mkrl = calculate_mrkl_coinbase_update(cbhx, &tarstf.mrklrts);
        tarstf.block.set_mrklroot(mkrl);
        let blkhx = tarstf.block.hash();
        if 1 == hash_diff(&blkhx, &tarstf.target_hash) {
            return api_error(&format!(
                "difficulty check fail: at least need {} but got {}",
                tarstf.target_hash.to_hex(),
                blkhx.to_hex()
            ));
        }
        let picked = stf.drain(stfidx..stfidx + 1).next_back().unwrap();
        picked
    };

    let done_height = success_stuff.block.height().uint();
    success_stuff
        .block
        .replace_transaction(0, Box::new(success_stuff.coinbase_tx.clone()))
        .unwrap();
    let blkpkg = BlkPkg::create(Box::new(success_stuff.block));
    if let Err(e) = ctx.hnoder.submit_block(&blkpkg, true) {
        return api_error(&format!("submit block error: {}", e));
    }
    api_ok(vec![
        ("height", json!(done_height)),
        ("mining", json!("success")),
    ])
}
