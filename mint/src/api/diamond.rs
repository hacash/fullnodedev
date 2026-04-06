fn diamond(ctx: &ApiExecCtx, req: ApiRequest) -> ApiResponse {
    let unit = q_string(&req, "unit", "fin");
    let mut name = q_string(&req, "name", "");
    let number = req.query_u64("number", 0) as u32;

    let staptr = read_mint_state(ctx);
    let state = CoreStateRead::wrap(staptr.as_ref().as_ref());
    if number > 0 {
        let Ok(dian) = DiamondNumber::from_usize(number as usize) else {
            return api_error("diamond number error");
        };
        let Some(dian) = state.diamond_name(&dian) else {
            return api_error("cannot find diamond");
        };
        name = dian.to_readable();
    } else if !DiamondName::is_valid(name.as_bytes()) {
        return api_error("invalid diamond name");
    }

    let raw: [u8; 6] = name.as_bytes().try_into().unwrap();
    let dian = DiamondName::from(raw);
    let Some(diaobj) = state.diamond(&dian) else {
        return api_error("cannot find diamond");
    };
    let Some(diasmelt) = state.diamond_smelt(&dian) else {
        return api_error("cannot find diamond");
    };

    let mut data = serde_json::Map::new();
    data.insert("name".to_owned(), json!(dian.to_readable()));
    data.insert("belong".to_owned(), json!(diaobj.address.to_readable()));
    data.insert("inscriptions".to_owned(), json!(diaobj.inscripts.array()));
    data.insert(
        "inscription_items".to_owned(),
        json!(
            diaobj
                .inscripts
                .as_list()
                .iter()
                .map(|item| json!({
                    "engraved_type": *item.engraved_type,
                    "content": item.to_readable_or_hex(),
                }))
                .collect::<Vec<_>>()
        ),
    );
    data.insert("number".to_owned(), json!(*diasmelt.number));
    data.insert(
        "miner".to_owned(),
        json!(diasmelt.miner_address.to_readable()),
    );
    data.insert(
        "born".to_owned(),
        json!({
            "height": *diasmelt.born_height,
            "hash": diasmelt.born_hash.to_hex(),
        }),
    );
    data.insert("prev_hash".to_owned(), json!(diasmelt.prev_hash.to_hex()));
    data.insert(
        "bid_fee".to_owned(),
        json!(diasmelt.bid_fee.to_unit_string(&unit)),
    );
    data.insert(
        "average_bid_burn".to_owned(),
        json!(*diasmelt.average_bid_burn),
    );
    data.insert("life_gene".to_owned(), json!(diasmelt.life_gene.to_hex()));
    data.insert(
        "visual_gene".to_owned(),
        json!(calculate_diamond_visual_gene(&dian, &diasmelt.life_gene).to_hex()),
    );
    api_data(data)
}
