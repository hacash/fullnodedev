fn latest(ctx: &ApiExecCtx, _req: ApiRequest) -> ApiResponse {
    let staptr = read_mint_state(ctx);
    let state = CoreStateRead::wrap(staptr.as_ref().as_ref());
    let lasthei = ctx.engine.latest_block().height().uint();
    let lastdia = state.get_latest_diamond();
    api_ok(vec![
        ("height", json!(lasthei)),
        ("diamond", json!(*lastdia.number)),
    ])
}
