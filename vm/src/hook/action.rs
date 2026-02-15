
pub fn try_action_hook(kid: u16, action: &dyn Any, ctx: &mut dyn Context, _gas: &mut u32) -> Rerr {

    use AbstCall::*;

    match kid {
        HacFromToTrs::KIND
        | HacFromTrs::KIND
        | HacToTrs::KIND
            => coin_asset_transfer_call(kid, PermitHAC, PayableHAC, action, ctx),
        | SatFromToTrs::KIND
        | SatFromTrs::KIND
        | SatToTrs::KIND
            => coin_asset_transfer_call(kid, PermitSAT, PayableSAT, action, ctx),
        | DiaSingleTrs::KIND
        | DiaFromToTrs::KIND
        | DiaFromTrs::KIND
        | DiaToTrs::KIND 
            => coin_asset_transfer_call(kid, PermitHACD, PayableHACD, action, ctx),
        | AssetFromToTrs::KIND
        | AssetFromTrs::KIND
        | AssetToTrs::KIND 
            => coin_asset_transfer_call(kid, PermitAsset, PayableAsset, action, ctx),
        _ => Ok(())
    }

}


fn coin_asset_transfer_call(kid: u16, abstfrom: AbstCall, abstto: AbstCall, action: &dyn Any, ctx: &mut dyn Context) -> Rerr {

    let addrs = &ctx.env().tx.addrs;
    let mut from = ctx.env().tx.main;
    let mut to = from.clone();
    let mut argvs: VecDeque<Value>;
    let absty = ExecMode::Abst as u8;
    let asset_param = |asset: &AssetAmt| {
        VecDeque::from([ 
            Value::U64(asset.serial.uint()), 
            Value::U64(asset.amount.uint()),
        ])
    };
    macro_rules! diamonds_param {
        ($act: expr) => {
            VecDeque::from([ Value::U32($act.diamonds.length() as u32), Value::Bytes($act.diamonds.form()) ])
        };
    }
    // HAC
    if let Some(act) = action.downcast_ref::<HacToTrs>() {
        to = act.to.real(addrs)?;
        argvs = VecDeque::from([Value::Bytes(act.hacash.serialize())]);
    }else if let Some(act) = action.downcast_ref::<HacFromTrs>() {
        from = act.from.real(addrs)?;
        argvs = VecDeque::from([Value::Bytes(act.hacash.serialize())]);
    }else if let Some(act) = action.downcast_ref::<HacFromToTrs>() {
        from = act.from.real(addrs)?;
        to = act.to.real(addrs)?;
        argvs = VecDeque::from([Value::Bytes(act.hacash.serialize())]);
    // SAT
    }else if let Some(act) = action.downcast_ref::<SatToTrs>() {
        to = act.to.real(addrs)?;
        argvs = VecDeque::from([Value::U64(act.satoshi.uint())]);
    }else if let Some(act) = action.downcast_ref::<SatFromTrs>() {
        from = act.from.real(addrs)?;
        argvs = VecDeque::from([Value::U64(act.satoshi.uint())]);
    }else if let Some(act) = action.downcast_ref::<SatFromToTrs>() {
        from = act.from.real(addrs)?;
        to = act.to.real(addrs)?;
        argvs = VecDeque::from([Value::U64(act.satoshi.uint())]);
    // HACD
    }else if let Some(act) = action.downcast_ref::<DiaSingleTrs>() {
        to = act.to.real(addrs)?;
        argvs = VecDeque::from([ Value::U32(1),  Value::Bytes(act.diamond.to_vec())]);
    }else if let Some(act) = action.downcast_ref::<DiaToTrs>() {
        to = act.to.real(addrs)?;
        argvs = diamonds_param!(act);
    }else if let Some(act) = action.downcast_ref::<DiaFromTrs>() {
        from = act.from.real(addrs)?;
        argvs = diamonds_param!(act);
    }else if let Some(act) = action.downcast_ref::<DiaFromToTrs>() {
        from = act.from.real(addrs)?;
        to = act.to.real(addrs)?;
        argvs = diamonds_param!(act);
    // Asset
    }else if let Some(act) = action.downcast_ref::<AssetToTrs>() {
        to = act.to.real(addrs)?;
        argvs = asset_param(&act.asset);
    }else if let Some(act) = action.downcast_ref::<AssetFromTrs>() {
        from = act.from.real(addrs)?;
        argvs = asset_param(&act.asset);
    }else if let Some(act) = action.downcast_ref::<AssetFromToTrs>() {
        from = act.from.real(addrs)?;
        to = act.to.real(addrs)?;
        argvs = asset_param(&act.asset);
    }else {
        unreachable!()
    }

    let (fs, fc, tc) = (from.is_scriptmh(), from.is_contract(), to.is_contract());
    if !(fs || fc || tc) {
        return Ok(()) // no script or contract address
    }

    const P2SH_PARAM_LEN: usize = 5; // witness + kind + to + (arg1, arg2)

    // call from p2sh script
    if fs {
        let p2sh = ctx.p2sh(&from)?;
        let witness = p2sh.witness().to_vec();
        let codes = p2sh.code_stuff().to_vec();
        let mut params: Vec<Value> = Vec::with_capacity(P2SH_PARAM_LEN);
        params.push(Value::Bytes(witness));
        params.push(Value::U16(kid));
        params.push(Value::Address(to));
        let mut args = argvs.clone();
        while params.len() < P2SH_PARAM_LEN {
            match args.pop_front() {
                Some(v) => params.push(v),
                None => params.push(Value::Nil),
            }
        }
        let param = Value::Compo(CompoItem::list(VecDeque::from(params))?);
        let cm = ExecMode::P2sh as u8;
        setup_vm_run(ctx, cm, 0, codes.into(), param)?;
        // return value checked inside p2sh_call
    }

    // call from contract abstract
    if fc {
        let mut argvs = argvs.clone();
        argvs.push_front( Value::Address(to) );
        let param = Value::Compo(CompoItem::list(argvs)?);
        setup_vm_run(ctx, absty, abstfrom as u8, Arc::from(from.as_bytes()), param)?;
        // return value checked inside abst_call
    }

    // call to contract abstract
    if tc {
        argvs.push_front( Value::Address(from) );
        let param = Value::Compo(CompoItem::list(argvs)?);
        setup_vm_run(ctx, absty, abstto as u8, Arc::from(to.as_bytes()), param)?;
        // return value checked inside abst_call
    }

    Ok(())
}
