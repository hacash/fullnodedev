/*
    if self.number.to_u32() > DIAMOND_ABOVE_NUMBER_OF_CREATE_BY_CUSTOM_MESSAGE {
        skn = self.custom_message.parse(buf, skn)?;
    }
*/
combi_struct_field_more_than_condition! { DiamondMintData, {
    diamond              : DiamondName
    number               : DiamondNumber
    prev_hash            : Hash
    nonce                : Fixed8
    address              : Address
}, custom_message, Hash, number, DIAMOND_ABOVE_NUMBER_OF_CREATE_BY_CUSTOM_MESSAGE
}

/*
* simple hac to
*/
action_define! { DiamondMint, 4,
    ActScope::TOP_ONLY, 2,
    *self.d.number > DIAMOND_ABOVE_NUMBER_OF_BURNING90_PERCENT_TX_FEES,
    [],
    {
        d: DiamondMintData
    },
    (self, format!("Mint diamond <{}> number {}", self.d.diamond.to_readable(), *self.d.number)),
    (self, ctx, _gas {
        diamond_mint(self, ctx)
    })
}

//
impl DiamondMint {
    pub fn with(diamond: DiamondName, number: DiamondNumber) -> Self {
        Self {
            kind: Uint2::from(Self::KIND),
            d: DiamondMintData {
                diamond,
                number,
                prev_hash: Hash::default(),
                nonce: Fixed8::default(),
                address: Address::default(),
                custom_message: Hash::default(),
            },
        }
    }
}

/*

*/
fn diamond_mint(this: &DiamondMint, ctx: &mut dyn Context) -> XRet<Vec<u8>> {
    let act = &this.d;
    act.address.must_privakey()?;
    let env = ctx.env().clone();
    let mut state = CoreState::wrap(ctx.state());
    let pending_height = env.block.height;
    let pending_hash = env.block.hash;
    let number = act.number;
    let dianum = *number;
    let name = act.diamond;
    let namestr = name.to_readable();
    let prev_hash = act.prev_hash;
    let nonce = act.nonce;
    let address = act.address;
    let mut custom_message = Vec::new();
    if dianum > DIAMOND_ABOVE_NUMBER_OF_CREATE_BY_CUSTOM_MESSAGE {
        custom_message = act.custom_message.serialize();
    }
    // check mine
    let (sha3hx, mediumhx, diahx) =
        x16rs::mine_diamond(dianum, &prev_hash, &nonce, &address, &custom_message);
    let not_fast_sync = !env.chain.fast_sync;
    if not_fast_sync {
        // check
        if pending_hash.not_zero() && pending_height % 5 != 0 {
            return xerrf!("diamond must be in a block height that is divisible by 5");
        }
        // number
        let latest_diamond = state.get_latest_diamond();
        let latestdianum = *latest_diamond.number;
        let neednextnumber = latestdianum + 1;
        if dianum != neednextnumber {
            return xerrf!(
                "diamond number expected {} but got {}",
                neednextnumber,
                dianum
            );
        }
        // check prev hash
        if dianum > 1 && latest_diamond.born_hash != prev_hash {
            return xerrf!(
                "diamond prev hash expected {} but got {}",
                latest_diamond.born_hash,
                prev_hash
            );
        }
        if dianum != 1 + latestdianum {
            return xerrf!(
                "latest diamond number expected {} but got {}",
                dianum - 1,
                latestdianum
            );
        }
        // check difficulty
        let diffok = x16rs::check_diamond_difficulty(dianum, &sha3hx, &mediumhx);
        if !diffok {
            return xerrf!("diamond difficulty does not match");
        }
        // name
        let dianame = x16rs::check_diamond_hash_result(diahx);
        let Some(dianame) = dianame else {
            let dhx = match String::from_utf8(diahx.to_vec()) {
                Err(_) => hex::encode(diahx),
                Ok(d) => d,
            };
            return xerrf!("diamond hash result {} is not a valid diamond name", dhx);
        };
        let dianame = Fixed6::from(dianame);
        if name != dianame {
            return xerrf!(
                "diamond name expected {} but got {}",
                dianame.to_readable(),
                namestr
            );
        }
        // exist
        let hav = state.diamond(&name);
        if let Some(_) = hav {
            return xerrf!("diamond {} already exists", namestr);
        }
    }
    // tx fee
    let tx_bid_fee = &env.tx.fee;
    // total count
    let mut ttcount = state.get_total_count();
    let minted = (*ttcount.minted_diamond as usize)
        .checked_add(1)
        .ok_or_else(|| "minted_diamond overflow".to_string())?;
    ttcount.minted_diamond = DiamondNumber::from_usize(minted)?;
    if dianum > DIAMOND_ABOVE_NUMBER_OF_BURNING90_PERCENT_TX_FEES {
        // Burn 90% by subtracting a safe floor(10%) part.
        let keep_ten_percent = tx_bid_fee.ten_percent_floor()?;
        let burn = tx_bid_fee.clone().sub_mode_u64(&keep_ten_percent)?;
        let burn_238 = burn.to_238_u64()?;
        let total_burn = (*ttcount.hacd_bid_burn_238)
            .checked_add(burn_238)
            .ok_or_else(|| "hacd_bid_burn_238 overflow".to_string())?;
        ttcount.hacd_bid_burn_238 = Uint8::from(total_burn);
    }
    // gene
    let (life_gene, _visual_gene) =
        calculate_diamond_gene(dianum, &mediumhx, &diahx, &pending_hash, &tx_bid_fee);
    // The running average here uses cumulative burned bid fee that already
    // includes the current diamond update in ttcount.
    let average_bid_burn = calculate_diamond_average_bid_burn(dianum, *ttcount.hacd_bid_burn_238);
    // save diamond smelt
    let diasmelt = DiamondSmelt {
        diamond: name.clone(),
        number: number.clone(),
        born_height: BlockHeight::from(pending_height),
        born_hash: pending_hash.clone(),
        prev_hash: prev_hash.clone(),
        miner_address: act.address.clone(),
        bid_fee: tx_bid_fee.clone(),
        nonce: nonce.clone(),
        average_bid_burn: average_bid_burn,
        life_gene: life_gene,
    };
    state.set_latest_diamond(&diasmelt);
    state.diamond_smelt_set(&name, &diasmelt);
    // save diamond
    let diaitem = DiamondSto {
        status: DIAMOND_STATUS_NORMAL,
        address: act.address.clone(),
        prev_engraved_height: BlockHeight::default(), // 0
        inscripts: Inscripts::default(),              // none
    };
    state.diamond_set(&name, &diaitem);
    state.diamond_name_set(&number, &name);
    // add diamond belong
    if env.chain.diamond_form {
        diamond_owned_push_one(&mut state, &address, &name);
    }
    // save count
    state.set_total_count(&ttcount);
    // add balance
    hacd_add(&mut state, &act.address, &DiamondNumber::from(1))?;
    // ok
    Ok(vec![])
}
