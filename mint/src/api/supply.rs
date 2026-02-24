fn supply(ctx: &ApiExecCtx, _req: ApiRequest) -> ApiResponse {
    let staptr = read_mint_state(ctx);
    let state = CoreStateRead::wrap(staptr.as_ref().as_ref());
    let lasthei = ctx.engine.latest_block().height().uint();
    const UNIT_238: u64 = 100_0000_0000;
    let supply = state.get_total_count();
    let blk_rwd = match cumulative_block_reward(lasthei).checked_mul(UNIT_238) {
        Some(v) => v,
        None => return api_error("block_reward overflow"),
    };
    let burned_diamond_bid = *supply.hacd_bid_burn_238;
    let burned_diamond_insc = *supply.diamond_insc_burn_238;
    let burned_tx_fee_90 = *supply.tx_fee_burn90_238;
    let burned_vm_ast_gas = *supply.ast_vm_gas_burn_238;
    let burned_asset_issue = *supply.asset_issue_burn_238;
    // `burned_diamond_bid` is a subset of `burned_tx_fee_90`, so do not add it again.
    let burn_fee = match burned_tx_fee_90
        .checked_add(burned_diamond_insc)
        .and_then(|v| v.checked_add(burned_vm_ast_gas))
        .and_then(|v| v.checked_add(burned_asset_issue))
    {
        Some(v) => v,
        None => return api_error("burned_fee overflow"),
    };
    let curr_ccl = match blk_rwd
        .checked_add(*supply.channel_interest_238)
        .and_then(|v| v.checked_sub(burn_fee))
    {
        Some(v) => v,
        None => return api_error("current_circulation overflow"),
    };
    let z2m = |v238| v238 as f64 / UNIT_238 as f64;
    api_ok(vec![
        ("latest_height", json!(lasthei)),
        ("current_circulation", json!(z2m(curr_ccl))),
        ("burned_fee", json!(z2m(burn_fee))),
        ("burned_diamond_bid", json!(z2m(burned_diamond_bid))),
        ("burned_diamond_insc", json!(z2m(burned_diamond_insc))),
        ("burned_tx_fee_90", json!(z2m(burned_tx_fee_90))),
        ("burned_ast_vm_gas", json!(z2m(burned_vm_ast_gas))),
        ("burned_asset_issue", json!(z2m(burned_asset_issue))),
        ("channel_deposit", json!(z2m(*supply.channel_deposit_238))),
        ("channel_interest", json!(z2m(*supply.channel_interest_238))),
        ("channel_opening", json!(*supply.opening_channel)),
        ("diamond_engraved", json!(*supply.diamond_engraved)),
        ("transferred_bitcoin", json!(0)),
        ("trsbtc_subsidy", json!(0)),
        ("block_reward", json!(z2m(blk_rwd))),
        ("minted_diamond", json!(*supply.minted_diamond)),
    ])
}
