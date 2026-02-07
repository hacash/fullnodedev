
api_querys_define!{ Q4396,
    __nnn__, Option<bool>, None,
    only_insert_txpool, Option<bool>, None,
}

/*

curl "http://127.0.0.1:8085/submit/transaction?hexbody=true" -X POST -d ""

*/
async fn submit_transaction(State(ctx): State<ApiCtx>, q: Query<Q4396>, body: Bytes) -> impl IntoResponse {
    let engcnf = ctx.engine.config(); 
    // body bytes
    let bddts = q_body_data_may_hex!(q, body);
    // println!("get tx body: {}", hex::encode(&bddts));
    // parse
    let txpkg = transaction::build_tx_package( bddts );
    if let Err(e) = txpkg {
        return api_error(&format!("transaction parse error: {}", &e))
    }
    let txpkg = txpkg.unwrap();
    // check sign
    if let Err(e) = txpkg.objc.verify_signature() {
        return api_error(&format!("transaction sign error: {}", &e))
    }
    // check fee
    if txpkg.fpur < engcnf.lowest_fee_purity { // fee_purity
        return api_error(&format!("The transaction fee purity {} is too low, the node minimum configuration is {}.", 
            txpkg.fpur, engcnf.lowest_fee_purity))
    }
    let txsz = txpkg.data.len();
    if txsz > engcnf.max_tx_size {
        return api_error(&format!("tx size cannot more than {} bytes", engcnf.max_tx_size));
    }
    // try submit
    let is_async = true;
    let only_insert_txpool = q.only_insert_txpool.unwrap_or(false);
    if let Err(e) = ctx.hcshnd.submit_transaction(&txpkg, is_async, only_insert_txpool) {
        return api_error(&e)
    }
    // ok
    api_data(jsondata!{
        "hash", txpkg.hash.to_hex(),
    })
}
