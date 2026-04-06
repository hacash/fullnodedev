fn vm_logs_del(ctx: &ApiExecCtx, req: ApiRequest) -> ApiResponse {
    let hei = req.query_u64("height", 0);
    let conf = ctx.engine.config();
    let auth_hash = conf.vm_log_delete_auth_hash.trim();
    if !auth_hash.is_empty() {
        let auth = req.query("auth").unwrap_or("").trim();
        if auth != auth_hash {
            return api_error("auth failed");
        }
    }
    ctx.engine.logs().remove(hei);
    api_data_raw(r#""ok":true"#.to_owned())
}
