// NTCTL.defer: Defer callback registration native function

fn extract_intent_handle_id_with_error(
    argv: &Value,
    err_code: ItrErrCode,
    err_msg: &str,
) -> VmrtRes<usize> {
    let Some(handle) = argv.match_handle() else {
        return itr_err_fmt!(err_code, "{}", err_msg);
    };
    let Some(intent_id) = handle.downcast_ref::<IntentId>() else {
        return itr_err_fmt!(err_code, "{}", err_msg);
    };
    Ok(intent_id.0)
}

fn defer_extract_intent_handle_id(argv: &Value) -> VmrtRes<usize> {
    extract_intent_handle_id_with_error(argv, DeferredError, "defer requires intent handle")
}

fn defer_intent_scope(
    argv: &Value,
    caddr: &crate::ContractAddress,
    intents: &crate::machine::IntentRuntime,
) -> VmrtRes<Option<Option<usize>>> {
    if argv.is_nil() {
        return Ok(Some(None));
    }
    let id = defer_extract_intent_handle_id(argv)?;
    intents
        .ensure_owner(caddr, id)
        .map_err(|e| ItrErr::new(DeferredError, &e.1))?;
    Ok(Some(Some(id)))
}

pub fn call_defer(
    exec: ExecCtx,
    bindings: &FrameBindings,
    intents: &mut crate::machine::IntentRuntime,
    deferred_registry: &mut crate::machine::DeferredRegistry,
    argv: Value,
) -> VmrtRes<(Value, i64)> {
    if exec.effect != EffectMode::Edit {
        return itr_err_fmt!(DeferredError, "defer not allowed in non-edit mode");
    }
    let Some(caddr) = bindings.state_this.clone() else {
        return itr_err_fmt!(DeferredError, "defer requires contract context");
    };
    let intent_scope = defer_intent_scope(&argv, &caddr, intents)?;
    deferred_registry.register(crate::machine::DeferredEntry {
        addr: caddr,
        intent_scope,
    })?;
    Ok((Value::Nil, NativeCtl::defer.gas_of()))
}
