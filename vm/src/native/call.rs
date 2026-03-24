pub fn call_ntfunc(hei: u64, idx: u8, argv: &[u8]) -> VmrtRes<(Value, i64)> {
    NativeFunc::call(hei, idx, argv)
}

pub fn call_ntctl(
    exec: ExecCtx,
    cap: &SpaceCap,
    bindings: &mut FrameBindings,
    intent_stack: &mut Vec<IntentBinding>,
    context_addr: &field::Address,
    intents: &mut crate::machine::IntentRuntime,
    deferred_registry: &mut crate::machine::DeferredRegistry,
    idx: u8,
    argv: Value,
) -> VmrtRes<(Value, i64)> {
    let ctl = NativeCtl::try_from_u8(idx)?;
    match ctl {
        NativeCtl::defer => call_defer(exec, context_addr, intents, deferred_registry, argv),
        NativeCtl::intent_new => call_intent_new(exec, intents, argv),
        NativeCtl::intent_use => call_intent_use(exec, cap, bindings, intent_stack, intents, argv),
        NativeCtl::intent_pop => call_intent_pop(exec, bindings, intent_stack, argv),
        NativeCtl::intent_put => call_intent_put(exec, bindings, intents, argv),
        NativeCtl::intent_get => call_intent_get(exec, bindings, intents, argv),
        NativeCtl::intent_take => call_intent_take(exec, bindings, intents, argv),
        _ => unreachable!(),
    }
}

pub fn call_ntenv(
    exec: ExecCtx,
    bindings: &FrameBindings,
    context_addr: &field::Address,
    idx: u8,
) -> VmrtRes<(Value, i64)> {
    if exec.effect == EffectMode::Pure {
        return itr_err_code!(InstDisabled);
    }
    let env = NativeEnv::try_from_u8(idx)?;
    let r = match env {
        NativeEnv::context_address => Value::Address(*context_addr),
        NativeEnv::intent_current => match bindings.intent_binding.flatten() {
            Some(id) => Value::U64(id),
            None => Value::Nil,
        },
        _ => unreachable!(),
    };
    Ok((r, env.gas_of()))
}
