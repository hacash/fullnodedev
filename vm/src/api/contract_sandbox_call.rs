fn contract_sandbox_call(ctx: &ApiExecCtx, req: ApiRequest) -> ApiResponse {
    let height = ctx.engine.latest_block().height().uint() + 1; // next height
    let engcnf = ctx.engine.config();
    let staptr = ctx.engine.state();
    let substa = staptr.fork_sub(Arc::downgrade(&staptr));
    let mut tx = TransactionType3::new_by(
        engcnf.external_exec_coinbase(),
        Amount::unit238(1_000_000),
        height,
    );
    tx.gas_max = Uint1::from(17);

    let env = Env {
        chain: ChainInfo {
            id: engcnf.chain_id,
            diamond_form: false,
            fast_sync: false,
        },
        block: BlkInfo {
            height,
            hash: Hash::default(),
            coinbase: engcnf.external_exec_coinbase(),
        },
        tx: protocol::transaction::create_tx_info(&tx),
    };
    let mut ctxobj = ContextInst::new(env, substa, Box::new(EmptyLogs {}), &tx);
    // One-shot sandbox context: created per request and dropped after this call.
    // `sandbox_call` may mutate runtime level/addrs and does not need to restore them.

    let contract = req.query("contract").unwrap_or("");
    let function = req.query("function").unwrap_or("").to_owned();
    let params = req.query("params").unwrap_or("");
    let Ok(addr) = Address::from_readable(contract) else {
        return api_error("contract address format invalid");
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
