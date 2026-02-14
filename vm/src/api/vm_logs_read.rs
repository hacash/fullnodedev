fn vm_logs_read(ctx: &ApiExecCtx, req: ApiRequest) -> ApiResponse {
    let ck_hei = req.query_u64("height", 0);
    let index = req.query_usize("index", 0);
    let rc_hei = ctx
        .engine
        .latest_block()
        .height()
        .uint()
        .saturating_sub(ctx.engine.config().unstable_block);
    if ck_hei > rc_hei {
        return api_data_raw(s!(r#""unstable":true"#));
    }
    let logs = ctx.engine.logs();
    let Some(itdts) = logs.load(ck_hei, index) else {
        return api_data_raw(s!(r#""end":true"#));
    };
    let Ok(item) = VmLog::build(&itdts).map_ire(ItrErrCode::LogError) else {
        return api_error("log format error");
    };
    let ignore = api_data_raw(s!(r#""ignore":true"#));
    if let Some(qadr) = req.query("address") {
        let Ok(addr) = req_addr(qadr) else {
            return api_error("address format error");
        };
        if addr != item.addr {
            return ignore;
        }
    }

    macro_rules! filter_topic {
        ($key:expr, $topic:expr) => {
            if let Some(tp) = req.query($key) {
                let Ok(raw) = req_hex(tp) else {
                    return api_error("hex format error");
                };
                if raw.as_slice() != $topic.raw() {
                    return ignore;
                }
            }
        };
    }

    filter_topic!("topic0", &item.topic0);
    filter_topic!("topic1", &item.topic1);
    filter_topic!("topic2", &item.topic2);
    filter_topic!("topic3", &item.topic3);

    api_data_raw(item.render(""))
}
