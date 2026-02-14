

pub fn routes() -> Router<ApiCtx> {

    let lrt = Router::new()
    
    // query
    .route(&query("coin/transfer"), get(scan_coin_transfer))

    .route(&query("block/intro"), get(block_intro))
    .route(&query("block/recents"), get(block_recents))
    .route(&query("block/views"), get(block_views))
    .route(&query("block/datas"), get(block_datas))

    .route(&query("transaction"), get(transaction_exist))

    .route(&query("fee/average"), get(fee_average))

    // create
    .route(&create("account"), get(account))
    .route(&create("transaction"), post(transaction_build))
    .route(&create("coin/transfer"), get(create_coin_transfer))
    
    // submit
    .route(&submit("transaction"), post(submit_transaction))
    .route(&submit("block"), post(submit_block))

    // operate
    .route(&operate("fee/raise"), post(fee_raise))

    // util
    .route(&util("transaction/check"), post(transaction_check))
    .route(&util("transaction/sign"), post(transaction_sign))

    ;

    lrt
    
}
