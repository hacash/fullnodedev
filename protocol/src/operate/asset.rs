

macro_rules! asset_operate_define {
    ($func_name: ident, $addr:ident, $amt:ident, $oldamt:ident,  $newsatblock:block) => (

        pub fn $func_name(state: &mut CoreState, $addr: &Address, $amt: &AssetAmt) -> XRet<AssetAmt> {
            if *$amt.amount == 0 {
                return xerrf!("Asset operate amount cannot be zero")
            }
            $addr.check_version()?;
            let mut userbls = state.balance( $addr ).unwrap_or_default();
            let $oldamt = &userbls.asset_must($amt.serial);
            /* -------- */
            let newast = $newsatblock;// operate
            /* -------- */
            // save
            userbls.asset_set(newast.clone())?;
            state.balance_set($addr, &userbls);
            Ok(newast)
        }

    )
}


/**************************** */

asset_operate_define!(asset_add, addr, asset, oldasset, {
    // do add
    oldasset.checked_add(asset)?
});

asset_operate_define!(asset_sub, addr, asset, oldasset, {  
    // check
    if oldasset < asset {
        return xerr_rf!("address {} asset {} is insufficient, at least {}", 
            addr, oldasset, asset)
    }
    // do sub
        oldasset.checked_sub(asset)?
});



/**************************** */


pub fn asset_transfer(ctx: &mut dyn Context, from: &Address, to: &Address, asset: &AssetAmt
) -> XRet<Vec<u8>> {
    if from == to {
		return xerrf!("cannot transfer to self")
    }
    // do transfer
    let state = &mut CoreState::wrap(ctx.state());
    asset_sub(state, from, asset)?;
    asset_add(state, to,   asset)?;
    blackhole_engulf(state, to);
    // ok
    Ok(vec![])
}


pub fn asset_check(ctx: &mut dyn Context, addr: &Address, ast: &AssetAmt) -> XRet<AssetAmt> {
    if *ast.amount == 0 {
        return xerrf!("asset check amount cannot be empty")
    }
    addr.check_version()?;
    let state = CoreState::wrap(ctx.state());
    if let Some(bls) = state.balance( addr ) {
        if let Some(uast) = bls.asset(ast.serial) {
            if uast >= *ast {
                return Ok(uast)
            }
        }
    }
    xerr_rf!("address {} asset is insufficient, at least {}", addr, ast)
}

