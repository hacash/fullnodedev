fn supply(ctx: &ApiExecCtx, _req: ApiRequest) -> ApiResponse {
    let staptr = read_mint_state(ctx);
    let state = CoreStateRead::wrap(staptr.as_ref().as_ref());
    let lasthei = ctx.engine.latest_block().height().uint();
    const UNIT_238: u128 = 100_0000_0000;
    let supply = state.get_total_count();
    let blk_rwd = match (cumulative_block_reward(lasthei) as u128).checked_mul(UNIT_238) {
        Some(v) => v,
        None => return api_error("block_reward overflow"),
    };
    let burned_diamond_bid = *supply.hacd_bid_burn_238;
    let burned_diamond_insc = *supply.diamond_insc_burn_238;
    let burned_legacy_tx_extra9 = *supply.tx_fee_burn90_238;
    let burned_vm_ast_gas = *supply.ast_vm_gas_burn_238;
    let burned_asset_issue = *supply.asset_issue_burn_238;
    let burned_contract_protocol_cost = *supply.contract_protocol_cost_burn_238;
    let burned_blackhole_hac = *supply.blackhole_hac_burn_238;
    // `burned_diamond_bid` is a subset of legacy tx extra9 burn, so do not add it again.
    let burn_fee = match burned_legacy_tx_extra9
        .checked_add(burned_diamond_insc)
        .and_then(|v| v.checked_add(burned_vm_ast_gas))
        .and_then(|v| v.checked_add(burned_asset_issue))
        .and_then(|v| v.checked_add(burned_contract_protocol_cost))
    {
        Some(v) => v,
        None => return api_error("burned_fee overflow"),
    };
    let curr_ccl = match blk_rwd
        .checked_add(*supply.channel_interest_238 as u128)
        .and_then(|v| v.checked_sub(burn_fee))
        .and_then(|v| v.checked_sub(burned_blackhole_hac))
    {
        Some(v) => v,
        None => return api_error("current_circulation overflow"),
    };
    let z2m = |v238: u128| v238 as f64 / UNIT_238 as f64;
    api_ok(vec![
        ("latest_height", json!(lasthei)),
        ("current_circulation", json!(z2m(curr_ccl))),
        ("burned_fee", json!(z2m(burn_fee))),
        ("burned_diamond_bid", json!(z2m(burned_diamond_bid))),
        ("burned_diamond_insc", json!(z2m(burned_diamond_insc))),
        ("burned_legacy_tx_extra9_fee", json!(z2m(burned_legacy_tx_extra9))),
        ("burned_ast_vm_gas", json!(z2m(burned_vm_ast_gas))),
        ("burned_asset_issue", json!(z2m(burned_asset_issue))),
        ("burned_contract_protocol_cost", json!(z2m(burned_contract_protocol_cost))),
        ("burned_blackhole_hac", json!(z2m(burned_blackhole_hac))),
        ("channel_deposit", json!(z2m(*supply.channel_deposit_238))),
        ("channel_interest", json!(z2m(*supply.channel_interest_238 as u128))),
        ("channel_opening", json!(*supply.opening_channel)),
        ("channel_open_total", json!(*supply.channel_open_total)),
        ("channel_close_total", json!(*supply.channel_close_total)),
        ("channel_closed_hac_volume", json!(z2m(*supply.channel_closed_hac_volume_238))),
        ("diamond_engraved", json!(*supply.diamond_engraved)),
        ("diamond_inscription_push_count", json!(*supply.dia_insc_push_count)),
        ("diamond_inscription_clean_count", json!(*supply.dia_insc_clean_count)),
        ("diamond_inscription_edit_count", json!(*supply.dia_insc_edit_count)),
        ("diamond_inscription_move_count", json!(*supply.dia_insc_move_count)),
        ("diamond_inscription_drop_count", json!(*supply.dia_insc_drop_count)),
        ("diamond_inscription_live_entry_count", json!(*supply.dia_insc_live_entry_count)),
        ("diamond_inscription_live_diamond_count", json!(*supply.dia_insc_live_diamond_count)),
        ("transferred_bitcoin", json!(0)),
        ("trsbtc_subsidy", json!(0)),
        ("block_reward", json!(z2m(blk_rwd))),
        ("minted_diamond", json!(*supply.minted_diamond)),
        ("contract_deploy_count", json!(*supply.contract_deploy_count)),
        ("contract_update_count", json!(*supply.contract_update_count)),
        ("contract_charge_bytes_total", json!(*supply.contract_charge_bytes_total)),
        ("tx_fee_pay_total", json!(z2m(*supply.tx_fee_pay_total_238))),
        ("tx_fee_got_total", json!(z2m(*supply.tx_fee_got_total_238))),
        ("blackhole_sat_burn", json!(*supply.blackhole_sat_burn)),
        ("blackhole_asset_burn_count", json!(*supply.blackhole_asset_burn_count)),
        ("blackhole_hacd_burn_count", json!(*supply.blackhole_hacd_burn_count)),
    ])
}
