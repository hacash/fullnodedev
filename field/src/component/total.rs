


/*
* TotalCount
*/
combi_struct!(TotalCount, 
	minted_diamond           : DiamondNumber
	hacd_bid_burn_238        : Uint8 // HAC unit238
	tx_fee_burn90_238        : Uint8 // legacy type1/2 tx extra9 burn, HAC unit238
	ast_vm_gas_burn_238      : Uint8 // HAC unit238
	// channel
	opening_channel          : Uint5
	channel_deposit_238      : Uint8 // HAC unit238
	channel_deposit_sat      : Uint8 // BTC sat
	channel_interest_238     : Uint8 // HAC unit238
	// diamond
	diamond_engraved         : Uint8
	diamond_insc_burn_238    : Uint8 // HAC unit238
	// asset
	created_asset            : Uint4
	asset_issue_burn_238     : Uint8 // HAC unit238
);
