fn submit_block(ctx: &ApiExecCtx, req: ApiRequest) -> ApiResponse {
    let Ok(bddts) = body_data_may_hex(&req) else {
        return api_error("block body invalid");
    };
    let blkpkg = build_block_package(bddts);
    let Ok(blkpkg) = blkpkg else {
        return api_error("block parse failed");
    };

    let is_async = true;
    if let Err(e) = ctx.hnoder.submit_block(&blkpkg, is_async) {
        return api_error(&format!("submit block failed: {}", e));
    }
    api_ok(vec![("ok", json!(true))])
}
