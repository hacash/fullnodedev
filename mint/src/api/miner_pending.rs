fn miner_pending(ctx: &ApiExecCtx, req: ApiRequest) -> ApiResponse {
    let detail = q_bool(&req, "detail", false);
    let transaction = q_bool(&req, "transaction", false);
    let stuff = q_bool(&req, "stuff", false);
    let base64 = q_bool(&req, "base64", false);

    if !ctx.engine.config().miner_enable {
        return api_error("miner not enable");
    }

    #[cfg(not(debug_assertions))]
    {
        let gotdmintx = ctx
            .hnoder
            .txpool()
            .first_at(TXGID_DIAMINT)
            .unwrap()
            .is_some();
        if ctx.engine.config().is_mainnet() && !gotdmintx && curtimes() < ctx.launch_time + 30 {
            return api_error("miner worker need launch after 30 secs for node start");
        }
    }

    let lasthei = ctx.engine.latest_block().height().uint();
    let need_create_new = {
        let stf = MINER_PENDING_BLOCK.lock().unwrap();
        if stf.is_empty() {
            true
        } else {
            *stf[0].height <= lasthei
        }
    };

    if need_create_new {
        miner_reset_next_new_block(ctx.engine.clone(), ctx.hnoder.txpool().as_ref());
    }

    get_miner_pending_block_stuff(detail, transaction, stuff, base64)
}
