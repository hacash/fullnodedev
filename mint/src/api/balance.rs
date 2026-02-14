fn balance(ctx: &ApiExecCtx, req: ApiRequest) -> ApiResponse {
    let unit = q_string(&req, "unit", "fin");
    let diamonds = q_bool(&req, "diamonds", false);
    let assets = q_bool(&req, "assets", false);
    let asset = req.query("asset").map(|s| s.to_owned());
    let (show_hacash, show_satoshi, show_diamond) = match q_coinkind_hsd(&req) {
        Ok(v) => v,
        Err(e) => return api_error(&e),
    };
    let ads = q_string(&req, "address", "")
        .replace(' ', "")
        .replace('\n', "");
    let addrs: Vec<_> = ads.split(',').collect();
    let adrsz = addrs.len();
    if adrsz == 0 || (adrsz == 1 && addrs[0].is_empty()) {
        return api_error("address format error");
    }
    if adrsz > 200 {
        return api_error("address max 200");
    }

    let staptr = read_mint_state(ctx);
    let core = CoreStateRead::wrap(staptr.as_ref().as_ref());
    let mut resbls = Vec::with_capacity(adrsz);
    for a in addrs {
        let Ok(adr) = Address::from_readable(a) else {
            return api_error(&format!("address {} format error", a));
        };
        let bls = core.balance(&adr).unwrap_or_default();
        let mut one = serde_json::Map::new();
        if show_hacash {
            one.insert("hacash".to_owned(), json!(bls.hacash.to_unit_string(&unit)));
        }
        if show_diamond {
            one.insert("diamond".to_owned(), json!(*bls.diamond));
        }
        if show_satoshi {
            one.insert("satoshi".to_owned(), json!(*bls.satoshi));
        }
        let mut resj = Value::Object(one);

        if diamonds && show_diamond {
            let diaowned = core.diamond_owned(&adr).unwrap_or_default();
            if let Some(obj) = resj.as_object_mut() {
                obj.insert("diamonds".to_owned(), json!(diaowned.readable()));
            }
        }

        if let Some(ast) = asset.as_ref() {
            let mut astlist = Vec::new();
            let mut astptr = &astlist;
            match ast.parse::<u64>() {
                Ok(astn) => {
                    if let Some(astobj) = bls.asset(Fold64::from(astn).unwrap_or(Fold64::max())) {
                        astlist.push(astobj);
                        astptr = &astlist;
                    }
                }
                _ => astptr = bls.assets.list(),
            };
            let mut arr = vec![];
            for it in astptr {
                arr.push(json!({
                    "serial": *it.serial,
                    "amount": *it.amount,
                }));
            }
            if let Some(obj) = resj.as_object_mut() {
                obj.insert("assets".to_owned(), json!(arr));
            }
        }

        if assets {
            let mut arr = vec![];
            for it in bls.assets.list() {
                arr.push(json!({
                    "serial": *it.serial,
                    "amount": *it.amount,
                }));
            }
            if let Some(obj) = resj.as_object_mut() {
                obj.insert("assets".to_owned(), json!(arr));
            }
        }

        resbls.push(resj);
    }
    api_data_list(resbls)
}
