
/*
* TotalCount
*
* Parse compatibility:
* - historical persisted state only includes the original leading fields
* - newly appended fields default to zero when absent during old-state replay
*/
combi_struct_with_parse!(TotalCount, (self, buf, {
	let mut mv = 0;
	let mut seek = buf;
	macro_rules! parse_req {
		($item:ident) => {
			mv += self.$item.parse_from(&mut seek)?;
		};
	}
	macro_rules! parse_opt {
		($item:ident) => {
			if !seek.is_empty() {
				mv += self.$item.parse_from(&mut seek)?;
			}
		};
	}
	parse_req!(minted_diamond);
	parse_req!(hacd_bid_burn_238);
	parse_req!(tx_fee_burn90_238);
	parse_req!(ast_vm_gas_burn_238);
	parse_req!(opening_channel);
	parse_req!(channel_deposit_238);
	parse_req!(channel_deposit_sat);
	parse_req!(channel_interest_238);
	parse_req!(diamond_engraved);
	parse_req!(diamond_insc_burn_238);
	parse_req!(created_asset);
	parse_req!(asset_issue_burn_238);
	parse_opt!(contract_protocol_cost_burn_238);
	parse_opt!(contract_deploy_count);
	parse_opt!(contract_update_count);
	parse_opt!(contract_charge_bytes_total);
	parse_opt!(tx_fee_pay_total_238);
	parse_opt!(tx_fee_got_total_238);
	parse_opt!(blackhole_hac_burn_238);
	parse_opt!(blackhole_sat_burn);
	parse_opt!(blackhole_asset_burn_count);
	parse_opt!(blackhole_hacd_burn_count);
	parse_opt!(channel_open_total);
	parse_opt!(channel_close_total);
	parse_opt!(channel_closed_hac_volume_238);
	parse_opt!(dia_insc_push_count);
	parse_opt!(dia_insc_clean_count);
	parse_opt!(dia_insc_edit_count);
	parse_opt!(dia_insc_move_count);
	parse_opt!(dia_insc_drop_count);
	parse_opt!(dia_insc_live_entry_count);
	parse_opt!(dia_insc_live_diamond_count);
	Ok(mv)
}),
	minted_diamond                : DiamondNumber
	hacd_bid_burn_238             : Uint12 // HAC unit238
	tx_fee_burn90_238             : Uint12 // legacy type1/2 tx extra9 burn, HAC unit238
	ast_vm_gas_burn_238           : Uint12 // HAC unit238
	// channel
	opening_channel               : Uint8
	channel_deposit_238           : Uint12 // HAC unit238
	channel_deposit_sat           : Uint8 // BTC sat
	channel_interest_238          : Uint8 // HAC unit238
	// diamond
	diamond_engraved              : Uint8
	diamond_insc_burn_238         : Uint12 // HAC unit238
	// asset
	created_asset                 : Uint8
	asset_issue_burn_238          : Uint12 // HAC unit238
	// contract
	contract_protocol_cost_burn_238 : Uint12 // HAC unit238
	contract_deploy_count         : Uint8
	contract_update_count         : Uint8
	contract_charge_bytes_total   : Uint12 // raw bytes
	// tx fee flow
	tx_fee_pay_total_238          : Uint12 // HAC unit238
	tx_fee_got_total_238          : Uint12 // HAC unit238
	// blackhole burn
	blackhole_hac_burn_238        : Uint12 // HAC unit238
	blackhole_sat_burn            : Uint8  // BTC sat
	blackhole_asset_burn_count    : Uint8  // burn events to ADDRESS_ZERO
	blackhole_hacd_burn_count     : Uint8  // HACD burn events to ADDRESS_ZERO
	// channel lifetime totals
	channel_open_total            : Uint8
	channel_close_total           : Uint8
	channel_closed_hac_volume_238 : Uint12 // HAC principal settled from channels, unit238
	// inscription actions and live set
	dia_insc_push_count           : Uint8
	dia_insc_clean_count          : Uint8
	dia_insc_edit_count           : Uint8
	dia_insc_move_count           : Uint8
	dia_insc_drop_count           : Uint8
	dia_insc_live_entry_count     : Uint8
	dia_insc_live_diamond_count   : Uint8
);

#[cfg(test)]
mod total_count_tests {
	use super::*;

	#[test]
	fn total_count_parse_legacy_bytes_defaults_new_fields_to_zero() {
		let mut old = Vec::new();
		DiamondNumber::from(3u32).serialize_to(&mut old);
		Uint12::from(7u128).serialize_to(&mut old);
		Uint12::from(11u128).serialize_to(&mut old);
		Uint12::from(13u128).serialize_to(&mut old);
		Uint8::from(17u64).serialize_to(&mut old);
		Uint12::from(19u128).serialize_to(&mut old);
		Uint8::from(23u64).serialize_to(&mut old);
		Uint8::from(29u64).serialize_to(&mut old);
		Uint8::from(31u64).serialize_to(&mut old);
		Uint12::from(37u128).serialize_to(&mut old);
		Uint8::from(41u64).serialize_to(&mut old);
		Uint12::from(43u128).serialize_to(&mut old);

		let got = TotalCount::build(&old).unwrap();
		assert_eq!(*got.minted_diamond, 3);
		assert_eq!(*got.channel_deposit_sat, 23);
		assert_eq!(*got.contract_deploy_count, 0);
		assert_eq!(*got.tx_fee_pay_total_238, 0);
		assert_eq!(*got.blackhole_hac_burn_238, 0);
		assert_eq!(*got.dia_insc_live_entry_count, 0);
	}
}
