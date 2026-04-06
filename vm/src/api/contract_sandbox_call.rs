fn contract_sandbox_call(ctx: &ApiExecCtx, req: ApiRequest) -> ApiResponse {
    let height = ctx.engine.latest_block().height().uint() + 1; // next height
    let engcnf = ctx.engine.config();
    let staptr = ctx.engine.state();
    let substa = staptr.fork_sub(Arc::downgrade(&staptr));
    let tx = TransactionType3::new_by(
        engcnf.external_exec_author(),
        Amount::unit238(1_000_000),
        height,
    );

    let env = Env {
        chain: ChainInfo {
            id: engcnf.chain_id,
            diamond_form: false,
            fast_sync: false,
        },
        block: BlkInfo {
            height,
            hash: Hash::default(),
            author: engcnf.external_exec_author(),
        },
        tx: protocol::transaction::create_tx_info(&tx),
    };
    let mut ctxobj = ContextInst::new(env, substa, Box::new(EmptyLogs {}), &tx);
    // One-shot sandbox context: created per request and dropped after this call.
    // `sandbox_call` may mutate runtime level/addrs and does not need to restore them.

    let contract = req.query("contract").unwrap_or("");
    let function = req.query("function").unwrap_or("").trim().to_owned();
    let params = req.query("params").unwrap_or("");
    let Ok(addr) = Address::from_readable(contract) else {
        return api_error("contract address format invalid");
    };
    let Ok(ctrladdr) = ContractAddress::from_addr(addr) else {
        return api_error("contract address version error");
    };
    if function.is_empty() {
        return api_error("function cannot be empty");
    }
    let caller = match req.query("caller") {
        Some(addr) => match req_addr(addr) {
            Ok(v) => Some(v),
            Err(_) => return api_error("caller address format invalid"),
        },
        None => None,
    };

    let args = match machine::parse_sandbox_params(params) {
        Ok(v) => v,
        Err(e) => return api_error(&e),
    };
    let mut spec = machine::SandboxSpec::new(ctrladdr, function).args(args);
    if let Some(caller) = caller {
        spec = spec.caller(caller);
    }
    let callres = match machine::sandbox_call(
        &mut ctxobj,
        spec,
    ) {
        Ok(v) => v,
        Err(e) => return api_error(&e),
    };
    api_data_raw(format!(
        r#""use_gas":{},"gas_use":{{"compute":{},"resource":{},"storage":{}}},"ret_val":{}"#,
        callres.use_gas,
        callres.gas_use.compute,
        callres.gas_use.resource,
        callres.gas_use.storage,
        callres.ret_val.to_debug_json()
    ))
}
