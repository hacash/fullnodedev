// NTCTL.defer: Defer callback registration native function

pub fn call_defer(
    exec: ExecCtx,
    context_addr: &field::Address,
    intents: &mut crate::machine::IntentRuntime,
    deferred_registry: &mut crate::machine::DeferredRegistry,
    argv: Value,
) -> VmrtRes<(Value, i64)> {
    if exec.effect != EffectMode::Edit {
        return itr_err_fmt!(
            DeferredError,
            "defer not allowed in non-edit mode"
        );
    }
    if exec.call_depth == 0 {
        return itr_err_fmt!(
            DeferredError,
            "defer not allowed at top-level entry"
        );
    }
    let caddr = crate::ContractAddress::from_addr(*context_addr).map_err(|e| {
        ItrErr::new(
            DeferredError,
            &format!("defer requires concrete contract frame: {}", e),
        )
    })?;
    let intent_id = if argv.is_nil() {
        None
    } else {
        let id = argv.extract_u64()?;
        if !intents.exists(id) {
            return itr_err_fmt!(DeferredError, "intent {} not found", id);
        }
        Some(id)
    };
    deferred_registry.register(crate::machine::DeferredEntry {
        addr: caddr,
        intent_id,
    })?;
    Ok((Value::Nil, NativeCtl::defer.gas_of()))
}
