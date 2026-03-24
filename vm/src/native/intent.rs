fn ctl_expect_no_arg(argv: Value, name: &str) -> VmrtErr {
    if argv.is_nil() {
        return Ok(());
    }
    itr_err_fmt!(
        ItrErrCode::IntentError,
        "native ctl '{}' expects no arguments",
        name
    )
}

fn ctl_require_edit(exec: ExecCtx, name: &str) -> VmrtErr {
    if exec.effect != EffectMode::Edit {
        return itr_err_fmt!(
            ItrErrCode::IntentError,
            "native ctl '{}' not allowed in non-edit mode",
            name
        );
    }
    Ok(())
}

fn ctl_require_main(exec: ExecCtx, name: &str) -> VmrtErr {
    ctl_require_edit(exec, name)?;
    if exec.entry != EntryKind::Main {
        return itr_err_fmt!(
            ItrErrCode::IntentError,
            "native ctl '{}' only allowed in main entry",
            name
        );
    }
    Ok(())
}

fn current_bound_intent(bindings: &FrameBindings) -> VmrtRes<u64> {
    bindings.intent_binding.flatten().ok_or_else(|| {
        ItrErr::new(
            ItrErrCode::IntentError,
            "current intent is not bound",
        )
    })
}

pub fn call_intent_new(
    exec: ExecCtx,
    intents: &mut crate::machine::IntentRuntime,
    argv: Value,
) -> VmrtRes<(Value, i64)> {
    ctl_require_main(exec, "intent_new")?;
    let id = intents.create(argv.extract_bytes()?)?;
    Ok((Value::U64(id), NativeCtl::intent_new.gas_of()))
}

pub fn call_intent_use(
    exec: ExecCtx,
    cap: &SpaceCap,
    bindings: &mut FrameBindings,
    intent_stack: &mut Vec<IntentBinding>,
    intents: &mut crate::machine::IntentRuntime,
    argv: Value,
) -> VmrtRes<(Value, i64)> {
    ctl_require_main(exec, "intent_use")?;
    if intent_stack.len() >= cap.intent_bind_depth_max {
        return itr_err_fmt!(
            ItrErrCode::IntentError,
            "intent bind depth exceeded max {}",
            cap.intent_bind_depth_max
        );
    }
    let binding = if argv.is_nil() {
        None
    } else {
        let id = argv.extract_u64()?;
        if !intents.exists(id) {
            return itr_err_fmt!(ItrErrCode::IntentError, "intent {} not found", id);
        }
        Some(id)
    };
    intent_stack.push(binding);
    bindings.intent_binding = intent_stack.last().cloned();
    Ok((Value::Nil, NativeCtl::intent_use.gas_of()))
}

pub fn call_intent_pop(
    exec: ExecCtx,
    bindings: &mut FrameBindings,
    intent_stack: &mut Vec<IntentBinding>,
    argv: Value,
) -> VmrtRes<(Value, i64)> {
    ctl_require_main(exec, "intent_pop")?;
    ctl_expect_no_arg(argv, "intent_pop")?;
    if intent_stack.pop().is_none() {
        return itr_err_fmt!(ItrErrCode::IntentError, "intent stack is empty");
    }
    bindings.intent_binding = intent_stack.last().cloned();
    Ok((Value::Nil, NativeCtl::intent_pop.gas_of()))
}

pub fn call_intent_put(
    exec: ExecCtx,
    bindings: &FrameBindings,
    intents: &mut crate::machine::IntentRuntime,
    argv: Value,
) -> VmrtRes<(Value, i64)> {
    ctl_require_edit(exec, "intent_put")?;
    let id = current_bound_intent(bindings)?;
    let Value::Tuple(args) = argv else {
        return itr_err_fmt!(ItrErrCode::IntentError, "intent_put requires 2 arguments");
    };
    if args.len() != 2 {
        return itr_err_fmt!(ItrErrCode::IntentError, "intent_put requires 2 arguments");
    }
    let items = args.to_vec();
    intents.put(
        id,
        Value::Bytes(items[0].extract_bytes()?),
        Value::Bytes(items[1].extract_bytes()?),
    )?;
    Ok((Value::Nil, NativeCtl::intent_put.gas_of()))
}

pub fn call_intent_get(
    exec: ExecCtx,
    bindings: &FrameBindings,
    intents: &mut crate::machine::IntentRuntime,
    argv: Value,
) -> VmrtRes<(Value, i64)> {
    if exec.effect == EffectMode::Pure {
        return itr_err_fmt!(
            ItrErrCode::IntentError,
            "native ctl 'intent_get' not allowed in pure mode"
        );
    }
    let id = current_bound_intent(bindings)?;
    let key = Value::Bytes(argv.extract_bytes()?);
    Ok((intents.get(id, &key)?, NativeCtl::intent_get.gas_of()))
}

pub fn call_intent_take(
    exec: ExecCtx,
    bindings: &FrameBindings,
    intents: &mut crate::machine::IntentRuntime,
    argv: Value,
) -> VmrtRes<(Value, i64)> {
    ctl_require_edit(exec, "intent_take")?;
    let id = current_bound_intent(bindings)?;
    let key = Value::Bytes(argv.extract_bytes()?);
    Ok((intents.take(id, &key)?, NativeCtl::intent_take.gas_of()))
}
