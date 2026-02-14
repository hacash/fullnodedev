fn fee_average(ctx: &ApiExecCtx, req: ApiRequest) -> ApiResponse {
    let unit = q_string(&req, "unit", "fin");
    let consumption = req.query_u64("consumption", 0);
    let burn90 = q_bool(&req, "burn90", false);
    let avgfeep = ctx.engine.average_fee_purity();

    let mut data = serde_json::Map::new();
    data.insert("purity".to_owned(), json!(avgfeep));

    if consumption > 0 {
        let mut setfee = Amount::unit238(avgfeep * consumption);
        if setfee.is_zero() {
            setfee = Amount::zhu(1);
        }
        if burn90 {
            if let Ok(f) = setfee.dist_mul(10) {
                setfee = f;
            }
        }
        data.insert("feasible".to_owned(), json!(setfee.to_unit_string(&unit)));
    }
    api_data(data)
}

fn fee_raise(ctx: &ApiExecCtx, req: ApiRequest) -> ApiResponse {
    let fee_s = q_string(&req, "fee", "");
    let fee_prikey = q_string(&req, "fee_prikey", "");
    let hash = q_string(&req, "hash", "");
    let Ok(fee) = Amount::from(&fee_s) else {
        return api_error("fee format error");
    };
    let Ok(acc) = Account::create_by(&fee_prikey) else {
        return api_error("fee_prikey format error");
    };

    let bddts = if !hash.is_empty() {
        let Ok(hx) = hex::decode(&hash) else {
            return api_error("hash parse error");
        };
        if hx.len() != Hash::SIZE {
            return api_error("hash size error");
        }
        let txhx = Hash::must(&hx);
        let txf = ctx.hnoder.txpool().find(&txhx);
        let Some(tx) = txf else {
            return api_error(&format!("cannot find tx by hash {} in tx pool", hash));
        };
        tx.data
    } else {
        let Ok(b) = body_data_may_hex(&req) else {
            return api_error("tx body error");
        };
        b.into()
    };

    let txb = protocol::transaction::transaction_create(&bddts);
    let Ok((mut txb, _)) = txb else {
        return api_error("transaction parse error");
    };

    let old_fee = txb.fee();
    if fee < *old_fee {
        return api_error(&format!("fee {} cannot less than old set {}", fee, old_fee));
    }
    txb.set_fee(fee.clone());
    if txb.fill_sign(&acc).is_err() {
        return api_error("fill sign error");
    }
    let txhash = txb.hash();
    let txhashwf = txb.hash_with_fee();
    let txpkg = TxPkg::create(txb);

    let is_async = true;
    if let Err(e) = ctx.hnoder.submit_transaction(&txpkg, is_async, false) {
        return api_error(&e);
    }

    api_data(serde_json::Map::from_iter([
        ("hash".to_owned(), json!(txhash.to_hex())),
        ("hash_with_fee".to_owned(), json!(txhashwf.to_hex())),
        ("fee".to_owned(), json!(fee.to_fin_string())),
        ("tx_body".to_owned(), json!(txpkg.objc.serialize().to_hex())),
    ]))
}
