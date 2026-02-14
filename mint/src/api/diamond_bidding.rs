fn diamond_bidding(ctx: &ApiExecCtx, req: ApiRequest) -> ApiResponse {
    let unit = q_string(&req, "unit", "fin");
    let limit = req.query_usize("limit", 20);
    let number = req.query_usize("number", 0) as u32;
    let since = q_bool(&req, "since", false);

    let staptr = read_mint_state(ctx);
    let state = CoreStateRead::wrap(staptr.as_ref().as_ref());
    let lastdia = state.get_latest_diamond();
    let txpool = ctx.hnoder.txpool();
    let mut datalist = vec![];

    let mut pick_dmint = |a: &TxPkg| {
        if datalist.len() >= limit {
            return false;
        }
        let txhx = a.hash;
        let txr = a.objc.as_ref().as_read();
        let Some(diamtact) = action::pickout_diamond_mint_action(txr) else {
            return true;
        };
        let act = diamtact.d;
        if number > 0 && number != *act.number {
            return true;
        }
        let mut one = json!({
            "tx": txhx.to_hex(),
            "fee": txr.fee().to_unit_string(&unit),
            "bid": txr.main().to_readable(),
            "name": act.diamond.to_readable(),
            "belong": act.address.to_readable(),
        });
        if number == 0 {
            one.as_object_mut()
                .unwrap()
                .insert("number".to_owned(), json!(*act.number));
        }
        datalist.push(one);
        true
    };
    txpool.iter_at(TXGID_DIAMINT, &mut pick_dmint).unwrap();

    let mut data = serde_json::Map::new();
    data.insert("number".to_owned(), json!(*lastdia.number + 1));
    data.insert("list".to_owned(), json!(datalist));

    if since {
        if let Ok(blk) = load_block_by_height(ctx, lastdia.born_height.uint()) {
            data.insert("since".to_owned(), json!(blk.objc.timestamp().uint()));
        }
    }
    api_data(data)
}
