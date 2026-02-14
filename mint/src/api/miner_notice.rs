fn miner_notice(ctx: &ApiExecCtx, req: ApiRequest) -> ApiResponse {
    let target_height = req.query_u64("height", 0);
    let mut wait = req.query_u64("wait", 45);
    set_in_range!(wait, 1, 300);
    let _mwnc = MWNCount::new(ctx.miner_worker_notice_count.clone());
    let mut lasthei;
    for _ in 0..wait {
        lasthei = ctx.engine.latest_block().height().uint();
        if lasthei >= target_height {
            break;
        }
        sleep(Duration::from_secs(1));
    }
    lasthei = ctx.engine.latest_block().height().uint();
    api_ok(vec![("height", json!(lasthei))])
}
