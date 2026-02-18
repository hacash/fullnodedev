fn diamond_views(ctx: &ApiExecCtx, req: ApiRequest) -> ApiResponse {
    let unit = q_string(&req, "unit", "fin");
    let mut limit = q_i64(&req, "limit", 20);
    let page = q_i64(&req, "page", 1);
    let start = q_i64(&req, "start", i64::MAX);
    let desc = q_bool(&req, "desc", false);
    let name = q_string(&req, "name", "");

    let staptr = read_mint_state(ctx);
    let state = CoreStateRead::wrap(staptr.as_ref().as_ref());
    let lastdianum = *state.get_latest_diamond().number as i64;
    if limit > 200 {
        limit = 200;
    }

    let mut datalist = vec![];
    let mut query_item = |dian: &DiamondName| {
        let Some(..) = state.diamond(dian) else {
            return;
        };
        let Some(diasmelt) = state.diamond_smelt(dian) else {
            return;
        };
        datalist.push(json!({
            "name": dian.to_readable(),
            "number": *diasmelt.number,
            "bid_fee": diasmelt.bid_fee.to_unit_string(&unit),
            "life_gene": diasmelt.life_gene.to_hex(),
        }));
    };

    if name.len() >= DiamondName::SIZE {
        let Ok(names) = DiamondNameListMax200::from_readable(&name) else {
            return api_error("diamond name error");
        };
        for dian in names.as_list() {
            query_item(&dian);
        }
    } else {
        for id in get_id_range(lastdianum, page, limit, start, desc) {
            let Some(dian) = state.diamond_name(&DiamondNumber::from(id as u32)) else {
                continue;
            };
            query_item(&dian);
        }
    }

    let mut data = serde_json::Map::new();
    data.insert("latest_number".to_owned(), json!(lastdianum));
    data.insert("list".to_owned(), json!(datalist));
    api_data(data)
}
