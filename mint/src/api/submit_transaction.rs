fn submit_transaction(ctx: &ApiExecCtx, req: ApiRequest) -> ApiResponse {
    let engcnf = ctx.engine.config();
    let Ok(bddts) = body_data_may_hex(&req) else {
        return api_error("transaction body error");
    };
    let txpkg = protocol::transaction::build_tx_package(bddts);
    let Ok(txpkg) = txpkg else {
        return api_error("transaction parse error");
    };

    if txpkg.fpur < engcnf.lowest_fee_purity {
        return api_error(&format!(
            "The transaction fee purity {} is too low, the node minimum configuration is {}.",
            txpkg.fpur, engcnf.lowest_fee_purity
        ));
    }
    let txsz = txpkg.data.len();
    if txsz > engcnf.max_tx_size {
        return api_error(&format!("tx size cannot more than {} bytes", engcnf.max_tx_size));
    }

    let is_async = true;
    let only_insert_txpool = q_bool(&req, "only_insert_txpool", false);
    if let Err(e) = ctx
        .hnoder
        .submit_transaction(&txpkg, is_async, only_insert_txpool)
    {
        return api_error(&e);
    }
    api_data(serde_json::Map::from_iter([(
        "hash".to_owned(),
        json!(txpkg.hash.to_hex()),
    )]))
}
