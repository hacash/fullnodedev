pub fn routes() -> Router<ApiCtx> {
    Router::new()
        .route(&query("coin/transfer"), get(scan_coin_transfer))
        .route(&query("latest"), get(latest))
        .route(&create("account"), get(account))
        .route(&create("coin/transfer"), get(create_coin_transfer))
}
