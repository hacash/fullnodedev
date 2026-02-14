fn diamondminer_init(ctx: &ApiExecCtx, _req: ApiRequest) -> ApiResponse {
    let cnf = ctx.engine.config();
    if !cnf.dmer_enable {
        return api_error("diamond miner in config not enable");
    }
    api_ok(vec![
        ("bid_address", json!(cnf.dmer_bid_account.readable())),
        (
            "reward_address",
            json!(cnf.dmer_reward_address.to_readable()),
        ),
    ])
}
