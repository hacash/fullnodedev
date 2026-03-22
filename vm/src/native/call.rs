pub fn call_ntfunc(hei: u64, idx: u8, argv: &[u8]) -> VmrtRes<(Value, i64)> {
    NativeFunc::call(hei, idx, argv)
}

pub fn call_ntctl(
    exec: ExecCtx,
    context_addr: &field::Address,
    deferred_registry: &mut crate::machine::DeferredRegistry,
    idx: u8,
) -> VmrtRes<(Value, i64)> {
    let ctl = NativeCtl::try_from_u8(idx)?;
    match ctl {
        NativeCtl::defer => call_defer(exec, context_addr, deferred_registry),
        _ => unreachable!(),
    }
}

pub fn call_ntenv(exec: ExecCtx, context_addr: &field::Address, idx: u8) -> VmrtRes<(Value, i64)> {
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
