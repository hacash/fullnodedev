fn submit_block(ctx: &ApiExecCtx, req: ApiRequest) -> ApiResponse {
    let Ok(bddts) = body_data_may_hex(&req) else {
        return api_error("block body error");
    };
    let blkpkg = build_block_package(bddts);
    let Ok(blkpkg) = blkpkg else {
        return api_error("block parse error");
    };

    let is_async = true;
    if let Err(e) = ctx.hnoder.submit_block(&blkpkg, is_async) {
        return api_error(&format!("submit block error: {}", e));
    }
    api_ok(vec![("ok", json!(true))])
}
