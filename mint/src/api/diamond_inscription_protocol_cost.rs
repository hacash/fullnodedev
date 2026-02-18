fn parse_one_diamond_param(req: &ApiRequest, key: &str) -> Ret<DiamondName> {
    let raw = q_string(req, key, "");
    let val = raw.trim();
    if val.is_empty() {
        return errf!("query '{}' cannot be empty", key);
    }
    let Ok(name) = DiamondName::from_readable(val.as_bytes()) else {
        return errf!("query '{}' diamond name format error", key);
    };
    Ok(name)
}

fn append_cost_for_one(state: &CoreStateRead, dia: &DiamondName) -> Ret<Amount> {
    let Some(diaobj) = state.diamond(dia) else {
        return errf!("cannot find diamond {}", dia);
    };
    if diaobj.inscripts.length() >= action::INSCRIPTION_MAX_PER_DIAMOND {
        return errf!(
            "diamond {} inscriptions full (max {})",
            dia.to_readable(),
            action::INSCRIPTION_MAX_PER_DIAMOND
        );
    }
    let Some(diasmelt) = state.diamond_smelt(dia) else {
        return errf!("cannot find diamond {}", dia);
    };
    Ok(action::calc_append_inscription_protocol_cost(
        diaobj.inscripts.length(),
        *diasmelt.average_bid_burn,
    ))
}

fn move_cost_for_target(state: &CoreStateRead, to_diamond: &DiamondName) -> Ret<Amount> {
    let Some(diaobj) = state.diamond(to_diamond) else {
        return errf!("cannot find diamond {}", to_diamond);
    };
    if diaobj.inscripts.length() >= action::INSCRIPTION_MAX_PER_DIAMOND {
        return errf!(
            "target diamond {} inscriptions full (max {})",
            to_diamond.to_readable(),
            action::INSCRIPTION_MAX_PER_DIAMOND
        );
    }
    let Some(diasmelt) = state.diamond_smelt(to_diamond) else {
        return errf!("cannot find diamond {}", to_diamond);
    };
    Ok(action::calc_move_inscription_protocol_cost(
        diaobj.inscripts.length(),
        *diasmelt.average_bid_burn,
    ))
}

fn edit_cost_for_one(state: &CoreStateRead, dia: &DiamondName) -> Ret<Amount> {
    let Some(_diaobj) = state.diamond(dia) else {
        return errf!("cannot find diamond {}", dia);
    };
    let Some(diasmelt) = state.diamond_smelt(dia) else {
        return errf!("cannot find diamond {}", dia);
    };
    Ok(action::calc_edit_inscription_protocol_cost(
        *diasmelt.average_bid_burn,
    ))
}

fn drop_cost_for_one(state: &CoreStateRead, dia: &DiamondName) -> Ret<Amount> {
    let Some(_diaobj) = state.diamond(dia) else {
        return errf!("cannot find diamond {}", dia);
    };
    let Some(diasmelt) = state.diamond_smelt(dia) else {
        return errf!("cannot find diamond {}", dia);
    };
    Ok(action::calc_drop_inscription_protocol_cost(
        *diasmelt.average_bid_burn,
    ))
}

fn add_amount(total: &mut Amount, add: &Amount) -> Rerr {
    let next = total.add_mode_u64(add)?;
    *total = next;
    Ok(())
}

fn diamond_inscription_protocol_cost_impl(
    ctx: &ApiExecCtx,
    req: ApiRequest,
    force_action: Option<&str>,
) -> ApiResponse {
    let unit = q_string(&req, "unit", "fin");
    let action_key = force_action
        .map(|v| v.to_owned())
        .unwrap_or_else(|| q_string(&req, "action", "append").to_lowercase());
    let staptr = read_mint_state(ctx);
    let state = CoreStateRead::wrap(staptr.as_ref().as_ref());

    let mut cost = Amount::new();
    let quote_res: Rerr = match action_key.as_str() {
        "append" => {
            let name = q_string(&req, "name", "");
            let Ok(names) = DiamondNameListMax200::from_readable(&name) else {
                return api_error("diamond name format or count error");
            };
            for dia in names.as_list() {
                let camt = match append_cost_for_one(&state, dia) {
                    Ok(v) => v,
                    Err(e) => return api_error(&e),
                };
                if let Err(e) = add_amount(&mut cost, &camt) {
                    return api_error(&e);
                }
            }
            Ok(())
        }
        "move" => {
            let to_diamond = match parse_one_diamond_param(&req, "to") {
                Ok(v) => v,
                Err(e) => return api_error(&e),
            };
            let from_raw = q_string(&req, "from", "");
            if !from_raw.trim().is_empty() {
                let from_diamond = match parse_one_diamond_param(&req, "from") {
                    Ok(v) => v,
                    Err(e) => return api_error(&e),
                };
                if from_diamond == to_diamond {
                    return api_error("source and target HACD cannot be the same");
                }
            }
            match move_cost_for_target(&state, &to_diamond) {
                Ok(v) => {
                    cost = v;
                    Ok(())
                }
                Err(e) => Err(e),
            }
        }
        "edit" => {
            let dia = match parse_one_diamond_param(&req, "name") {
                Ok(v) => v,
                Err(e) => return api_error(&e),
            };
            match edit_cost_for_one(&state, &dia) {
                Ok(v) => {
                    cost = v;
                    Ok(())
                }
                Err(e) => Err(e),
            }
        }
        "drop" => {
            let dia = match parse_one_diamond_param(&req, "name") {
                Ok(v) => v,
                Err(e) => return api_error(&e),
            };
            match drop_cost_for_one(&state, &dia) {
                Ok(v) => {
                    cost = v;
                    Ok(())
                }
                Err(e) => Err(e),
            }
        }
        _ => return api_error("action must be append/move/edit/drop"),
    };
    if let Err(e) = quote_res {
        return api_error(&e);
    }

    let mut data = serde_json::Map::new();
    data.insert("action".to_owned(), json!(action_key));
    data.insert("cost".to_owned(), json!(cost.to_unit_string(&unit)));
    api_data(data)
}

fn diamond_inscription_protocol_cost(ctx: &ApiExecCtx, req: ApiRequest) -> ApiResponse {
    diamond_inscription_protocol_cost_impl(ctx, req, None)
}

fn diamond_inscription_protocol_cost_append(ctx: &ApiExecCtx, req: ApiRequest) -> ApiResponse {
    diamond_inscription_protocol_cost_impl(ctx, req, Some("append"))
}

fn diamond_inscription_protocol_cost_move(ctx: &ApiExecCtx, req: ApiRequest) -> ApiResponse {
    diamond_inscription_protocol_cost_impl(ctx, req, Some("move"))
}

fn diamond_inscription_protocol_cost_edit(ctx: &ApiExecCtx, req: ApiRequest) -> ApiResponse {
    diamond_inscription_protocol_cost_impl(ctx, req, Some("edit"))
}

fn diamond_inscription_protocol_cost_drop(ctx: &ApiExecCtx, req: ApiRequest) -> ApiResponse {
    diamond_inscription_protocol_cost_impl(ctx, req, Some("drop"))
}
