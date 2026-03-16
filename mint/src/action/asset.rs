

pub const ASSET_ALIVE_HEIGHT: u64 = 700000;


#[allow(unused)]
fn check_alive_blk_hei(ctx: &mut dyn Context) -> Ret<(u64, u64)> {
    let chei = ctx.env().block.height;
    let is_mainnet = ctx.env().chain.id==0 && chei >= ASSET_ALIVE_HEIGHT;
    let alive_hei: u64 = maybe!(is_mainnet, ASSET_ALIVE_HEIGHT, 0);
    let minsri:    u64 = maybe!(is_mainnet, 1025, 5);
    Ok((alive_hei, minsri))
}

    // By design tx.main plus protocol fee authorizes asset creation, while metadata.issuer is only the initial allocation target and does not need to sign.
action_define!{ AssetCreate, 16, 
    ActScope::TOP_ONLY, false, [], 
    {
        metadata: AssetSmelt
        protocol_fee: Amount
    },
    (self, format!("Register asset <{}>", self.metadata.ticket)),
    (self, ctx, _gas {
        let amd = self.metadata.clone();
        let serial = *amd.serial;
        // check serial
        let chei = ctx.env().block.height;
        let (alive_hei, minsri) = check_alive_blk_hei(ctx)?;
        if alive_hei > chei {
            return err!("The asset issuance has not yet begun")
        }
        if serial < minsri {
            return errf!("serial cannot be less than {}", minsri)
        }
        let serial_limit = chei - alive_hei;
        if serial > serial_limit {
            return err!("asset serial overflow")
        }
        // check meta
        amd.issuer.check_version()?;
        let tl = amd.ticket.length();
        let nl = amd.name.length();
        if tl < 1 || tl > 8 {
            return err!("ticket length must be 1 ~ 8")
        }
        if nl < 1 || nl > 32 {
            return err!("name length must be 1 ~ 32")
        }
        if !check_readable_string(&amd.ticket) {
            return err!("ticket must be ascii2 readable string")
        }
        if !check_readable_string(&amd.name) {
            return err!("name must be ascii2 readable string")
        }
        if *amd.decimal > 16 {
            return err!("decimal cannot exceed 16")
        }
        if amd.supply.is_zero() {
            return err!("supply must be greater than zero")
        }
        // check fee and burn
        let blkrw = super::genesis::block_reward(chei);
        let pfee = self.protocol_fee.clone();
        if pfee != blkrw {
            return errf!("Protocol fee must be {} but got {}", blkrw, pfee)
        }
	    // sub main addr balance for protocol fee
        let main_addr = ctx.env().tx.main; 
        hac_sub(ctx, &main_addr, &pfee)?;
        // state and check exists
        let mut sta = CoreState::wrap(ctx.state());
        if let Some(..) = sta.asset(&amd.serial) {
            return errf!("Asset serial {} already exists", serial)
        }
        sta.asset_set(&amd.serial, &amd); // store asset object
        // total count update
        let mut ttcount = sta.get_total_count();
        let new_created_asset = ttcount.created_asset.uint()
            .checked_add(1)
            .ok_or("created_asset overflow".to_string())?;
        ttcount.created_asset = Uint4::from(new_created_asset);
        let pfee_238 = pfee.to_238_u64()?;
        let new_asset_issue_burn_238 = (*ttcount.asset_issue_burn_238)
            .checked_add(pfee_238)
            .ok_or("asset_issue_burn_238 overflow".to_string())?;
        ttcount.asset_issue_burn_238 = Uint8::from(new_asset_issue_burn_238);
        sta.set_total_count(&ttcount);
        // do mint
        let asset_obj = AssetAmt::from(amd.serial.uint(), amd.supply.uint())?;
        // issue
        let mut bls = sta.balance(&amd.issuer).unwrap_or_default();
        bls.asset_set(asset_obj)?;
        sta.balance_set(&amd.issuer, &bls);
        // ok finish
        Ok(vec![])
    })
}
