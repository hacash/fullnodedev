fn diamond_inscription_protocol_cost(ctx: &ApiExecCtx, req: ApiRequest) -> ApiResponse {
    let unit = q_string(&req, "unit", "fin");
    let name = q_string(&req, "name", "");
    let Ok(names) = DiamondNameListMax200::from_readable(&name) else {
        return api_error("diamond name format or count error");
    };

    let staptr = read_mint_state(ctx);
    let state = CoreStateRead::wrap(staptr.as_ref().as_ref());
    let mut cost = Amount::new();
    for dia in names.list() {
        let Some(diaobj) = state.diamond(dia) else {
            return api_error(&format!("cannot find diamond {}", dia));
        };
        if diaobj.inscripts.length() < 10 {
            continue;
        }
        let Some(diasmelt) = state.diamond_smelt(dia) else {
            return api_error(&format!("cannot find diamond {}", dia));
        };
        let camt = Amount::coin(*diasmelt.average_bid_burn as u64, 247);
        let Ok(newcost) = cost.add_mode_u128(&camt) else {
            return api_error(&format!("cannot add cost {} and {}", camt, cost));
        };
        cost = newcost;
    }

    let mut data = serde_json::Map::new();
    data.insert("cost".to_owned(), json!(cost.to_unit_string(&unit)));
    api_data(data)
}
