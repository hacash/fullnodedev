fn supply(ctx: &ApiExecCtx, _req: ApiRequest) -> ApiResponse {
    let staptr = read_mint_state(ctx);
    let state = CoreStateRead::wrap(staptr.as_ref().as_ref());
    let lasthei = ctx.engine.latest_block().height().uint();
    let lastdia = state.get_latest_diamond();
    const ZHU: u64 = 1_0000_0000;
    let supply = state.get_total_count();
    let blk_rwd = cumulative_block_reward(lasthei) * ZHU;
    let burn_fee = *supply.hacd_bid_burn_zhu + *supply.diamond_insc_burn_zhu;
    let curr_ccl = blk_rwd + *supply.channel_interest_zhu - burn_fee;
    let z2m = |zhu| zhu as f64 / ZHU as f64;
    api_ok(vec![
        ("latest_height", json!(lasthei)),
        ("current_circulation", json!(z2m(curr_ccl))),
        ("burned_fee", json!(z2m(burn_fee))),
        ("burned_diamond_bid", json!(z2m(*supply.hacd_bid_burn_zhu))),
        ("channel_deposit", json!(z2m(*supply.channel_deposit_zhu))),
        ("channel_interest", json!(z2m(*supply.channel_interest_zhu))),
        ("channel_opening", json!(*supply.opening_channel)),
        ("diamond_engraved", json!(*supply.diamond_engraved)),
        ("transferred_bitcoin", json!(0)),
        ("trsbtc_subsidy", json!(0)),
        ("block_reward", json!(z2m(blk_rwd))),
        ("minted_diamond", json!(*lastdia.number)),
    ])
}
