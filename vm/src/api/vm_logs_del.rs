fn vm_logs_del(ctx: &ApiExecCtx, req: ApiRequest) -> ApiResponse {
    let hei = req.query_u64("height", 0);
    ctx.engine.logs().remove(hei);
    api_data_raw(r#""ok":true"#.to_owned())
}
