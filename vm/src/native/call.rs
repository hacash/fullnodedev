pub fn call_ntfunc(hei: u64, idx: u8, argv: &[u8]) -> VmrtRes<(Value, i64)> {
    NativeFunc::call(hei, idx, argv)
}

pub fn call_ntctl(
    exec: ExecCtx,
    cap: &SpaceCap,
    bindings: &mut FrameBindings,
    intent_state: &mut crate::frame::IntentBindingState,
    _context_addr: &field::Address,
    intents: &mut crate::machine::IntentRuntime,
    deferred_registry: &mut crate::machine::DeferredRegistry,
    idx: u8,
    argv: Value,
) -> VmrtRes<(Value, i64)> {
    let ctl = NativeCtl::try_from_u8(idx)?;
    macro_rules! call_intent {
        ($name: ident) => {
            concat_idents::concat_idents!{  f_call_intent = call_intent_, $name {
                f_call_intent(exec, bindings, intent_state, intents, argv)
            }}
        }
    }
    match ctl {
        NativeCtl::defer => call_defer(exec, bindings, intents, deferred_registry, argv),
        NativeCtl::intent_new           => call_intent!{ new },
        NativeCtl::intent_use => call_intent_use(exec, cap, bindings, intent_state, intents, argv),
        NativeCtl::intent_pop => call_intent_pop(exec, bindings, intent_state, argv),
        NativeCtl::intent_is_own_handle        => call_intent!{ is_own_handle },
        NativeCtl::intent_kind          => call_intent!{ kind },
        NativeCtl::intent_kind_is       => call_intent!{ kind_is },
        NativeCtl::intent_destroy       => call_intent!{ destroy },
        NativeCtl::intent_destroy_if_empty => call_intent!{ destroy_if_empty },
        NativeCtl::intent_clear         => call_intent!{ clear },
        NativeCtl::intent_len           => call_intent!{ len },
        NativeCtl::intent_has           => call_intent!{ has },
        NativeCtl::intent_keys          => call_intent!{ keys },
        NativeCtl::intent_keys_page     => call_intent!{ keys_page },
        NativeCtl::intent_keys_after     => call_intent!{ keys_after },
        NativeCtl::intent_get           => call_intent!{ get },
        NativeCtl::intent_get_or        => call_intent!{ get_or },
        NativeCtl::intent_require       => call_intent!{ require },
        NativeCtl::intent_require_eq    => call_intent!{ require_eq },
        NativeCtl::intent_require_absent => call_intent!{ require_absent },
        NativeCtl::intent_require_many  => call_intent!{ require_many },
        NativeCtl::intent_require_map   => call_intent!{ require_map },
        NativeCtl::intent_has_all       => call_intent!{ has_all },
        NativeCtl::intent_has_any       => call_intent!{ has_any },
        NativeCtl::intent_put           => call_intent!{ put },
        NativeCtl::intent_put_if_absent => call_intent!{ put_if_absent },
        NativeCtl::intent_put_if_absent_or_match => call_intent!{ put_if_absent_or_match },
        NativeCtl::intent_put_flat_kv     => call_intent!{ put_flat_kv },
        NativeCtl::intent_replace       => call_intent!{ replace },
        NativeCtl::intent_replace_if    => call_intent!{ replace_if },
        NativeCtl::intent_rename          => call_intent_rename(exec, bindings, intent_state, intents, argv),
        NativeCtl::intent_take          => call_intent!{ take },
        NativeCtl::intent_take_or       => call_intent!{ take_or },
        NativeCtl::intent_take_if       => call_intent!{ take_if },
        NativeCtl::intent_take_many     => call_intent!{ take_many },
        NativeCtl::intent_take_map      => call_intent!{ take_map },
        NativeCtl::intent_consume       => call_intent!{ consume },
        NativeCtl::intent_consume_many  => call_intent!{ consume_many },
        NativeCtl::intent_del           => call_intent!{ del },
        NativeCtl::intent_del_if        => call_intent!{ del_if },
        NativeCtl::intent_del_many      => call_intent!{ del_many },
        NativeCtl::intent_append        => call_intent!{ append },
        NativeCtl::intent_inc           => call_intent!{ inc },
        NativeCtl::intent_add           => call_intent!{ add },
        NativeCtl::intent_sub           => call_intent!{ sub },
        _ => unreachable!(),
    }
}

pub fn call_ntenv(
    exec: ExecCtx,
    _bindings: &FrameBindings,
    context_addr: &field::Address,
    idx: u8,
) -> VmrtRes<(Value, i64)> {
    if exec.effect == EffectMode::Pure {
        return itr_err_code!(InstDisabled);
    }
    let env = NativeEnv::try_from_u8(idx)?;
    let r = match env {
        NativeEnv::context_address => Value::Address(*context_addr),
        _ => unreachable!(),
    };
    Ok((r, env.gas_of()))
}
