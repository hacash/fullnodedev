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
    if exec.call_depth == 0 {
        return itr_err_fmt!(DeferredError, "defer not allowed at top-level entry");
    }
    let caddr = match bindings.state_this.clone() {
        Some(caddr) => caddr,
        None => crate::ContractAddress::from_addr(bindings.context_addr).map_err(|e| {
            ItrErr::new(
                DeferredError,
                &format!("defer requires concrete contract frame: {}", e),
            )
        })?,
    };
    let intent_id = if argv.is_nil() {
        None
    } else {
        let id = defer_extract_intent_handle_id(&argv)?;
        intents
            .ensure_owner(&caddr, id)
            .map_err(|e| ItrErr::new(DeferredError, &e.1))?;
        Some(id)
    };
    deferred_registry.register(crate::machine::DeferredEntry {
        addr: caddr,
        intent_id,
    })?;
    Ok((Value::Nil, NativeCtl::defer.gas_of()))
}
