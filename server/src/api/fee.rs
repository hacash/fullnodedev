

/******************** fee average ********************/


api_querys_define!{ Q7365,
    consumption, Option<u64>, None,  // tx size or gas use
    burn90,      Option<bool>, None, // if tx burn 90% fee
}

async fn fee_average(State(ctx): State<ApiCtx>, q: Query<Q7365>) -> impl IntoResponse {
    q_unit!(q, unit);
    q_must!(q, consumption, 0);
    q_must!(q, burn90, false);

    let avgfeep = ctx.engine.average_fee_purity(); // unit: 238

    let mut data = jsondata!{
        "purity", avgfeep, // 238
    };

    if consumption > 0 {
        // fee_purity is per-byte (unit-238), consumption is tx size in bytes
        let mut setfee = Amount::unit238(avgfeep * consumption);
        if setfee.is_zero() {
            setfee = Amount::zhu(1);
        }
        if burn90 {
            if let Ok(f) = setfee.dist_mul(10) {
                setfee = f;
            }
        }
        data.insert("feasible", json!(setfee.to_unit_string(&unit)));
    }
    // ok
    api_data(data)
}


/******************** raise fee ********************/

api_querys_define!{ Q5396,
    fee, String, s!(""),
    fee_prikey, String, s!(""),
    hash, Option<String>, None, // find by tx hash
}

async fn fee_raise(State(ctx): State<ApiCtx>, q: Query<Q5396>, body: Bytes) -> impl IntoResponse {
    // ctx_store!(ctx, store);
    q_must!(q, hash, s!(""));
    let fee = q_data_amt!(q, fee);
    let acc = q_data_acc!(q, fee_prikey);

    let txhxstr = &hash;
    let bddts = maybe!(txhxstr.len() > 0, {
        // find from tx pool
        let txhx = q_data_hash!(txhxstr);
        let txf = ctx.hcshnd.txpool().find(&txhx);
        let Some(tx) = txf else {
            return api_error(&format!("cannot find tx by hash {} in tx pool", &txhxstr))
        };
        tx.data
    }, {
        // tx body data
        q_body_data_may_hex!(q, body).into()
    });
    
    // parse
    let txb = transaction::transaction_create(&bddts);
    if let Err(e) = txb {
        return api_error(&format!("transaction parse error: {}", &e))
    }
    let (mut txb, _) = txb.unwrap();

    // check set fee
    let old_fee = txb.fee();
    if fee < *old_fee {
        return api_error(&format!("fee {} cannot less than old set {}", fee, old_fee))
    }
    txb.set_fee(fee.clone());
    txb.fill_sign(&acc).unwrap();
    if let Err(e) = txb.verify_signature() {
        return api_error(&format!("transaction signature verify error: {}", &e))
    }
    let txhash = txb.hash();
    let txhashwf = txb.hash_with_fee();
    // pkg
    let txpkg = TxPkg::create(txb);
    // submit tx & add to txpool
    let is_async = true;
    if let Err(e) = ctx.hcshnd.submit_transaction(&txpkg, is_async, false) {
        return api_error(&e)
    }
    // ok
    let data = jsondata!{
        "hash", txhash.to_hex(),
        "hash_with_fee", txhashwf.to_hex(),
        "fee", fee.to_fin_string(),
        "tx_body", txpkg.objc.serialize().to_hex(),
    };
    api_data(data)
}