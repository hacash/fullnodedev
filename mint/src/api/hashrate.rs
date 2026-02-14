fn hashrate(ctx: &ApiExecCtx, _req: ApiRequest) -> ApiResponse {
    api_data(query_hashrate(ctx))
}
