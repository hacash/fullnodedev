fn load_block_by_key(ctx: &ApiExecCtx, key: &str) -> Ret<Arc<BlkPkg>> {
    let store = ctx.engine.store();
    if key.len() == Hash::SIZE * 2 {
        if let Ok(hx) = hex::decode(key) {
            if hx.len() == Hash::SIZE {
                let hash = Hash::must(&hx);
                if let Some(blkdts) = store.block_data(&hash) {
                    let blkpkg = build_block_package(blkdts).map_err(|e| format!("block parse error: {}", e))?;
                    return Ok(Arc::new(blkpkg));
                }
            }
        }
    }
    if let Ok(height) = key.parse::<u64>() {
        if let Some((_, blkdts)) = store.block_data_by_height(&BlockHeight::from(height)) {
            let blkpkg = build_block_package(blkdts).map_err(|e| format!("block parse error: {}", e))?;
            return Ok(Arc::new(blkpkg));
        }
    }
    errf!("block not find")
}

fn api_bytes(data: Vec<u8>, content_type: &str) -> ApiResponse {
    ApiResponse {
        status: 200,
        headers: vec![("content-type".to_owned(), content_type.to_owned())],
        body: data,
    }
}

fn block_intro(ctx: &ApiExecCtx, req: ApiRequest) -> ApiResponse {
    let unit = q_string(&req, "unit", "fin");
    let mut key = q_string(&req, "hash", "");
    let height = q_u32(&req, "height", 0);
    let tx_hash_list = q_bool(&req, "tx_hash_list", false);
    if height > 0 {
        key = height.to_string();
    }
    let Ok(blkpkg) = load_block_by_key(ctx, &key) else {
        return api_error("cannot find block");
    };
    let blkobj = &blkpkg.objc;
    let cbtx = create_recent_block_info(blkobj.as_read());

    let txnum = blkobj.transaction_count().uint() as usize - 1;
    let mut data = serde_json::Map::new();
    data.insert("hash".to_owned(), json!(blkpkg.hash.to_hex()));
    data.insert("version".to_owned(), json!(blkobj.version().uint()));
    data.insert("height".to_owned(), json!(blkobj.height().uint()));
    data.insert("timestamp".to_owned(), json!(blkobj.timestamp().uint()));
    data.insert("mrklroot".to_owned(), json!(blkobj.mrklroot().to_hex()));
    data.insert("prevhash".to_owned(), json!(blkobj.prevhash().to_hex()));
    data.insert("nonce".to_owned(), json!(blkobj.nonce().uint()));
    data.insert("difficulty".to_owned(), json!(blkobj.difficulty().uint()));
    data.insert("miner".to_owned(), json!(cbtx.miner.to_readable()));
    data.insert("reward".to_owned(), json!(cbtx.reward.to_unit_string(&unit)));
    data.insert("message".to_owned(), json!(cbtx.message));
    data.insert("transaction".to_owned(), json!(txnum));

    if tx_hash_list {
        let alltrs = blkobj.transactions();
        let mut txhxs = Vec::with_capacity(txnum);
        for tx in &alltrs[1..txnum + 1] {
            txhxs.push(tx.hash().to_hex());
        }
        data.insert("tx_hash_list".to_owned(), json!(txhxs));
    }
    api_data(data)
}

fn block_recents(ctx: &ApiExecCtx, req: ApiRequest) -> ApiResponse {
    let unit = q_string(&req, "unit", "fin");
    let mut datalist = vec![];
    for li in ctx.engine.recent_blocks() {
        datalist.push(json!({
            "height": li.height,
            "hash": li.hash.to_hex(),
            "prev": li.prev.to_hex(),
            "txs": li.txs.saturating_sub(1),
            "miner": li.miner.to_readable(),
            "message": li.message,
            "reward": li.reward.to_unit_string(&unit),
            "time": li.time,
            "arrive": li.arrive,
        }));
    }
    api_data(serde_json::Map::from_iter([
        ("list".to_owned(), json!(datalist)),
    ]))
}

fn block_views(ctx: &ApiExecCtx, req: ApiRequest) -> ApiResponse {
    let unit = q_string(&req, "unit", "fin");
    let mut limit = q_i64(&req, "limit", 20);
    let page = q_i64(&req, "page", 1);
    let start = q_i64(&req, "start", i64::MAX);
    let desc = q_bool(&req, "desc", false);
    if limit > 200 {
        limit = 200;
    }

    let store = ctx.engine.store();
    let lasthei = ctx.engine.latest_block().height().uint() as i64;
    let mut datalist = vec![];
    for id in get_id_range(lasthei, page, limit, start, desc) {
        let Some((blkhx, blkdts)) = store.block_data_by_height(&BlockHeight::from(id as u64)) else {
            continue;
        };
        let dts = blkdts.as_ref();
        let Ok((intro, seek)) = BlockIntro::create(dts) else {
            continue;
        };
        let Ok((cbtx, _)) = TransactionCoinbase::create(&dts[seek..]) else {
            continue;
        };
        datalist.push(json!({
            "height": intro.height().uint(),
            "hash": blkhx.to_hex(),
            "msg": cbtx.message().to_readable_left(),
            "reward": cbtx.reward().to_unit_string(&unit),
            "miner": cbtx.main().to_readable(),
            "time": intro.timestamp().uint(),
            "txs": intro.transaction_count().uint().saturating_sub(1),
        }));
    }
    api_data(serde_json::Map::from_iter([
        ("latest_height".to_owned(), json!(lasthei)),
        ("list".to_owned(), json!(datalist)),
    ]))
}

fn block_datas(ctx: &ApiExecCtx, req: ApiRequest) -> ApiResponse {
    let store = ctx.engine.store();
    let unsblk = ctx.engine.config().unstable_block;
    let mut lasthei = ctx.engine.latest_block().height().uint();

    const MB: usize = 1024 * 1024;
    let hexbody = q_bool(&req, "hexbody", false);
    let base64body = q_bool(&req, "base64body", false);
    let start_height = req.query_u64("start_height", 0);
    let limit = req.query_u64("limit", u64::MAX);
    let mut max_size = req.query_usize("max_size", MB);
    let confirm = q_bool(&req, "confirm", false);
    if max_size > 10 * MB {
        max_size = 10 * MB;
    }
    if confirm && lasthei > unsblk {
        lasthei -= unsblk;
    }

    let mut alldatas = Vec::with_capacity(max_size);
    let mut count = 0u64;
    for hei in start_height..u64::MAX {
        if hei > lasthei || count >= limit || alldatas.len() >= max_size {
            break;
        }
        let Some((_, mut blkdts)) = store.block_data_by_height(&BlockHeight::from(hei)) else {
            break;
        };
        alldatas.append(&mut blkdts);
        count += 1;
    }

    let content_type = if hexbody || base64body {
        "text/plain; charset=utf-8"
    } else {
        "application/octet-stream"
    };
    if hexbody {
        return api_bytes(alldatas.to_hex().into_bytes(), content_type);
    }
    if base64body {
        return api_bytes(alldatas.to_base64().into_bytes(), content_type);
    }
    api_bytes(alldatas, content_type)
}
