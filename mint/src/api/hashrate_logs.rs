fn hashrate_logs(ctx: &ApiExecCtx, req: ApiRequest) -> ApiResponse {
    let mut days = req.query_u64("days", 200);
    if days == 0 {
        days = 1;
    }
    let target = q_bool(&req, "target", false);
    let scale = q_f64(&req, "scale", 0.0);

    let mtcnf = ctx.engine.minter().config().downcast::<MintConf>().unwrap();
    let bac = mtcnf.difficulty_adjust_blocks;

    if days > 500 {
        return api_error("param days cannot more than 500");
    }
    let lasthei = ctx.engine.latest_block().height().uint();
    if lasthei < days {
        return api_error("param days overflow");
    }
    let secs = lasthei / days;

    let mut day200 = Vec::with_capacity(days as usize);
    let mut dayall = Vec::with_capacity(days as usize);
    let mut day200_max = 0u128;
    let mut dayall_max = 0u128;
    for i in 0..days {
        let s1 = lasthei - ((days - 1 - i) * bac);
        let s2 = secs + secs * i;
        let rt1 = get_blk_rate(ctx, s1).unwrap_or(0);
        let rt2 = get_blk_rate(ctx, s2).unwrap_or(0);
        if rt1 > day200_max {
            day200_max = rt1;
        }
        if rt2 > dayall_max {
            dayall_max = rt2;
        }
        day200.push(rt1);
        dayall.push(rt2);
    }

    if scale > 0.0 {
        if day200_max > 0 {
            let sd2 = day200_max as f64 / scale;
            for it in day200.iter_mut() {
                *it = (*it as f64 / sd2) as u128;
            }
        }
        if dayall_max > 0 {
            let sda = dayall_max as f64 / scale;
            for it in dayall.iter_mut() {
                *it = (*it as f64 / sda) as u128;
            }
        }
    }

    let mut data = serde_json::Map::new();
    if target {
        data = query_hashrate(ctx);
    }
    data.insert("day200".to_owned(), json!(day200));
    data.insert("dayall".to_owned(), json!(dayall));
    api_data(data)
}
