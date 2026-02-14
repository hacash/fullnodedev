fn contract_sandbox_call(ctx: &ApiExecCtx, req: ApiRequest) -> ApiResponse {
    let height = ctx.engine.latest_block().height().uint() + 1; // next height
    let engcnf = ctx.engine.config();
    let staptr = ctx.engine.state();
    let substa = staptr.fork_sub(Arc::downgrade(&staptr));
    let tx = TransactionType3::default();

    let env = Env {
        chain: ChainInfo {
            id: engcnf.chain_id,
            diamond_form: false,
            fast_sync: false,
        },
        block: BlkInfo {
            height,
            hash: Hash::default(),
            coinbase: Address::default(),
        },
        tx: protocol::transaction::create_tx_info(&tx),
    };
    let mut ctxobj = ContextInst::new(env, substa, Box::new(EmptyLogs {}), &tx);

    let contract = req.query("contract").unwrap_or("");
    let function = req.query("function").unwrap_or("").to_owned();
    let params = req.query("params").unwrap_or("");
    let Ok(addr) = Address::from_readable(contract) else {
        return api_error("contract address format error");
    };
    let Ok(ctrladdr) = ContractAddress::from_addr(addr) else {
        return api_error("contract address version error");
    };

    let callres = machine::sandbox_call(&mut ctxobj, ctrladdr, function, params);
    let Ok((gasuse, retval)) = callres else {
        return api_error("contract call error");
    };
    api_data_raw(format!(r#""gasuse":{},"return":{}"#, gasuse, retval))
}
