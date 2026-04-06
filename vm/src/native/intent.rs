use std::collections::VecDeque;

// Legacy helper functions (kept for compatibility with special cases)

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

fn ctl_require_non_pure(exec: ExecCtx, name: &str) -> VmrtErr {
    if exec.effect == EffectMode::Pure {
        return itr_err_fmt!(
            ItrErrCode::IntentError,
            "native ctl '{}' not allowed in pure mode",
            name
        );
    }
    Ok(())
}

fn ctl_contract_owner(bindings: &FrameBindings, name: &str) -> VmrtRes<crate::ContractAddress> {
    bindings.state_this.clone().ok_or_else(|| {
        ItrErr::new(
            ItrErrCode::IntentError,
            &format!("intent '{}' only allowed in contract context", name),
        )
    })
}

fn bound_intent_id(intent_state: &crate::frame::IntentScopeState, name: &str) -> VmrtRes<usize> {
    intent_state.current_bound_intent_id().ok_or_else(|| {
        ItrErr::new(
            ItrErrCode::IntentError,
            &format!("intent '{}' requires bound intent", name),
        )
    })
}

fn extract_intent_handle_id(argv: &Value, name: &str) -> VmrtRes<usize> {
    let err_msg = format!("native ctl '{}' requires intent handle", name);
    extract_intent_handle_id_with_error(argv, ItrErrCode::IntentError, &err_msg)
}

fn owned_bound_intent(
    bindings: &FrameBindings,
    intent_state: &crate::frame::IntentScopeState,
    intents: &crate::machine::IntentRuntime,
    name: &str,
) -> VmrtRes<(crate::ContractAddress, usize)> {
    let owner = ctl_contract_owner(bindings, name)?;
    let id = bound_intent_id(intent_state, name)?;
    intents.ensure_owner(&owner, id)?;
    Ok((owner, id))
}

fn ctl_expect_tuple(argv: Value, name: &str, expect: usize) -> VmrtRes<Vec<Value>> {
    let Value::Tuple(args) = argv else {
        return itr_err_fmt!(
            ItrErrCode::IntentError,
            "native ctl '{}' requires {} arguments",
            name,
            expect
        );
    };
    if args.len() != expect {
        return itr_err_fmt!(
            ItrErrCode::IntentError,
            "native ctl '{}' requires {} arguments",
            name,
            expect
        );
    }
    Ok(args.to_vec())
}

fn ctl_expect_list(argv: Value, name: &str) -> VmrtRes<Vec<Value>> {
    let compo = argv.compo_ref().map_err(|_| {
        ItrErr::new(
            ItrErrCode::IntentError,
            &format!("native ctl '{}' requires list argument", name),
        )
    })?;
    let list = compo.list_ref().map_err(|_| {
        ItrErr::new(
            ItrErrCode::IntentError,
            &format!("native ctl '{}' requires list argument", name),
        )
    })?;
    Ok(list.iter().cloned().collect())
}

fn ctl_expect_kv_list(argv: Value, name: &str) -> VmrtRes<Vec<(Value, Value)>> {
    let items = ctl_expect_list(argv, name)?;
    if items.len() % 2 != 0 {
        return itr_err_fmt!(
            ItrErrCode::IntentError,
            "native ctl '{}' requires list(key,value,...)",
            name
        );
    }
    let mut pairs = Vec::with_capacity(items.len() / 2);
    let mut iter = items.into_iter();
    while let Some(key) = iter.next() {
        let val = iter.next().unwrap();
        pairs.push((key, val));
    }
    Ok(pairs)
}

fn ctl_expect_bytes(value: &Value, name: &str, label: &str) -> VmrtRes<Vec<u8>> {
    value.extract_bytes().map_err(|_| {
        ItrErr::new(
            ItrErrCode::IntentError,
            &format!("native ctl '{}' requires {} bytes argument", name, label),
        )
    })
}

fn ctl_expect_u32(value: &Value, name: &str, label: &str) -> VmrtRes<u32> {
    value.extract_u32().map_err(|_| {
        ItrErr::new(
            ItrErrCode::IntentError,
            &format!("native ctl '{}' requires {} uint-family argument convertible to u32", name, label),
        )
    })
}

fn ctl_put_kv_list(
    exec: ExecCtx,
    bindings: &FrameBindings,
    intent_state: &crate::frame::IntentScopeState,
    intents: &mut crate::machine::IntentRuntime,
    argv: Value,
    name: &str,
) -> VmrtErr {
    ctl_require_edit(exec, name)?;
    let pairs = ctl_expect_kv_list(argv, name)?;
    let (owner, id) = owned_bound_intent(bindings, intent_state, intents, name)?;
    intents.put_many(&owner, id, pairs)
}

macro_rules! intent_std_fn {
    ($name:ident, |$exec:ident, $bindings:ident, $intent_state:ident, $intents:ident, $argv:ident| $body:block) => {
        pub fn $name(
            $exec: ExecCtx,
            $bindings: &FrameBindings,
            $intent_state: &crate::frame::IntentScopeState,
            $intents: &mut crate::machine::IntentRuntime,
            $argv: Value,
        ) -> VmrtRes<(Value, i64)> $body
    };
}

macro_rules! intent_stack_fn {
    ($name:ident, |$exec:ident, $cap:ident, $bindings:ident, $intent_state:ident, $intents:ident, $argv:ident| $body:block) => {
        pub fn $name(
            $exec: ExecCtx,
            $cap: &SpaceCap,
            $bindings: &mut FrameBindings,
            $intent_state: &mut crate::frame::IntentScopeState,
            $intents: &mut crate::machine::IntentRuntime,
            $argv: Value,
        ) -> VmrtRes<(Value, i64)> $body
    };
}

macro_rules! intent_pop_fn {
    ($name:ident, |$exec:ident, $bindings:ident, $intent_state:ident, $argv:ident| $body:block) => {
        pub fn $name(
            $exec: ExecCtx,
            $bindings: &mut FrameBindings,
            $intent_state: &mut crate::frame::IntentScopeState,
            $argv: Value,
        ) -> VmrtRes<(Value, i64)> $body
    };
}

intent_std_fn!(call_intent_new, |exec, bindings, _intent_state, intents, argv| {
    ctl_require_edit(exec, "intent_new")?;
    let owner = ctl_contract_owner(bindings, "intent_new")?;
    let id = intents.create(owner, ctl_expect_bytes(&argv, "intent_new", "kind")?)?;
    let handle = Value::handle(IntentId(id));
    Ok((handle, NativeCtl::intent_new.gas_of()))
});

intent_stack_fn!(call_intent_use, |exec, cap, _bindings, intent_state, intents, argv| {
    ctl_require_edit(exec, "intent_use")?;
    if intent_state.len() >= cap.intent_bind_depth {
        return itr_err_fmt!(
            ItrErrCode::IntentError,
            "intent bind depth exceeded max {}",
            cap.intent_bind_depth
        );
    }
    let binding: BoundIntentId = if argv.is_nil() {
        None
    } else {
        let id = extract_intent_handle_id(&argv, "intent_use")?;
        if !intents.exists(id) {
            return itr_err_fmt!(ItrErrCode::IntentError, "intent {} not found", id);
        }
        Some(id)
    };
    intent_state.push(binding);
    Ok((Value::Nil, NativeCtl::intent_use.gas_of()))
});

intent_pop_fn!(call_intent_pop, |exec, _bindings, intent_state, argv| {
    ctl_require_edit(exec, "intent_pop")?;
    ctl_expect_no_arg(argv, "intent_pop")?;
    if intent_state.pop().is_none() {
        return itr_err_fmt!(ItrErrCode::IntentError, "intent stack is empty");
    }
    Ok((Value::Nil, NativeCtl::intent_pop.gas_of()))
});

intent_std_fn!(call_intent_put, |exec, bindings, intent_state, intents, argv| {
    ctl_require_edit(exec, "intent_put")?;
    let (owner, id) = owned_bound_intent(bindings, intent_state, intents, "intent_put")?;
    let items = ctl_expect_tuple(argv, "intent_put", 2)?;
    intents.put(&owner, id, items[0].clone(), items[1].clone())?;
    Ok((Value::Nil, NativeCtl::intent_put.gas_of()))
});

intent_std_fn!(call_intent_get, |exec, bindings, intent_state, intents, argv| {
    ctl_require_non_pure(exec, "intent_get")?;
    let (owner, id) = owned_bound_intent(bindings, intent_state, intents, "intent_get")?;
    Ok((
        intents.get(&owner, id, &argv)?,
        NativeCtl::intent_get.gas_of(),
    ))
});

intent_std_fn!(call_intent_take, |exec, bindings, intent_state, intents, argv| {
    ctl_require_edit(exec, "intent_take")?;
    let (owner, id) = owned_bound_intent(bindings, intent_state, intents, "intent_take")?;
    Ok((
        intents.take(&owner, id, &argv)?,
        NativeCtl::intent_take.gas_of(),
    ))
});

intent_std_fn!(call_intent_del, |exec, bindings, intent_state, intents, argv| {
    ctl_require_edit(exec, "intent_del")?;
    let (owner, id) = owned_bound_intent(bindings, intent_state, intents, "intent_del")?;
    intents.del(&owner, id, &argv)?;
    Ok((Value::Nil, NativeCtl::intent_del.gas_of()))
});

intent_std_fn!(call_intent_has, |exec, bindings, intent_state, intents, argv| {
    ctl_require_non_pure(exec, "intent_has")?;
    let (owner, id) = owned_bound_intent(bindings, intent_state, intents, "intent_has")?;
    Ok((
        Value::Bool(intents.has(&owner, id, &argv)?),
        NativeCtl::intent_has.gas_of(),
    ))
});

intent_std_fn!(call_intent_kind, |exec, bindings, intent_state, intents, argv| {
    ctl_require_non_pure(exec, "intent_kind")?;
    ctl_expect_no_arg(argv, "intent_kind")?;
    let (owner, id) = owned_bound_intent(bindings, intent_state, intents, "intent_kind")?;
    Ok((intents.kind(&owner, id)?, NativeCtl::intent_kind.gas_of()))
});

intent_std_fn!(call_intent_kind_is, |exec, bindings, intent_state, intents, argv| {
    ctl_require_non_pure(exec, "intent_kind_is")?;
    let (owner, id) = owned_bound_intent(bindings, intent_state, intents, "intent_kind_is")?;
    let kind = ctl_expect_bytes(&argv, "intent_kind_is", "kind")?;
    Ok((
        Value::Bool(intents.kind_is(&owner, id, &kind)?),
        NativeCtl::intent_kind_is.gas_of(),
    ))
});

intent_std_fn!(call_intent_clear, |exec, bindings, intent_state, intents, argv| {
    ctl_require_edit(exec, "intent_clear")?;
    ctl_expect_no_arg(argv, "intent_clear")?;
    let (owner, id) = owned_bound_intent(bindings, intent_state, intents, "intent_clear")?;
    intents.clear_data(&owner, id)?;
    Ok((Value::Nil, NativeCtl::intent_clear.gas_of()))
});

intent_std_fn!(call_intent_len, |exec, bindings, intent_state, intents, argv| {
    ctl_require_non_pure(exec, "intent_len")?;
    ctl_expect_no_arg(argv, "intent_len")?;
    let (owner, id) = owned_bound_intent(bindings, intent_state, intents, "intent_len")?;
    Ok((
        Value::U64(intents.len(&owner, id)? as u64),
        NativeCtl::intent_len.gas_of(),
    ))
});

intent_std_fn!(call_intent_keys, |exec, bindings, intent_state, intents, argv| {
    ctl_require_non_pure(exec, "intent_keys")?;
    ctl_expect_no_arg(argv, "intent_keys")?;
    let (owner, id) = owned_bound_intent(bindings, intent_state, intents, "intent_keys")?;
    let keys = intents
        .keys_sorted(&owner, id)?
        .into_iter()
        .map(Value::Bytes)
        .collect::<VecDeque<_>>();
    Ok((
        Value::Compo(CompoItem::list(keys)?),
        NativeCtl::intent_keys.gas_of(),
    ))
});

intent_std_fn!(call_intent_get_or, |exec, bindings, intent_state, intents, argv| {
    ctl_require_non_pure(exec, "intent_get_or")?;
    let items = ctl_expect_tuple(argv, "intent_get_or", 2)?;
    let (owner, id) = owned_bound_intent(bindings, intent_state, intents, "intent_get_or")?;
    Ok((
        intents.get_or(&owner, id, &items[0], items[1].clone())?,
        NativeCtl::intent_get_or.gas_of(),
    ))
});

intent_std_fn!(call_intent_require, |exec, bindings, intent_state, intents, argv| {
    ctl_require_non_pure(exec, "intent_require")?;
    let (owner, id) = owned_bound_intent(bindings, intent_state, intents, "intent_require")?;
    Ok((
        intents.require(&owner, id, &argv)?,
        NativeCtl::intent_require.gas_of(),
    ))
});

intent_std_fn!(call_intent_require_eq, |exec, bindings, intent_state, intents, argv| {
    ctl_require_non_pure(exec, "intent_require_eq")?;
    let items = ctl_expect_tuple(argv, "intent_require_eq", 2)?;
    let (owner, id) = owned_bound_intent(bindings, intent_state, intents, "intent_require_eq")?;
    Ok((
        intents.require_eq(&owner, id, &items[0], &items[1])?,
        NativeCtl::intent_require_eq.gas_of(),
    ))
});

intent_std_fn!(call_intent_take_or, |exec, bindings, intent_state, intents, argv| {
    ctl_require_edit(exec, "intent_take_or")?;
    let items = ctl_expect_tuple(argv, "intent_take_or", 2)?;
    let (owner, id) = owned_bound_intent(bindings, intent_state, intents, "intent_take_or")?;
    let val = if intents.has(&owner, id, &items[0])? {
        intents.take(&owner, id, &items[0])?
    } else {
        items[1].clone()
    };
    Ok((val, NativeCtl::intent_take_or.gas_of()))
});

intent_std_fn!(call_intent_replace, |exec, bindings, intent_state, intents, argv| {
    ctl_require_edit(exec, "intent_replace")?;
    let items = ctl_expect_tuple(argv, "intent_replace", 2)?;
    let (owner, id) = owned_bound_intent(bindings, intent_state, intents, "intent_replace")?;
    let old = intents.replace(&owner, id, items[0].clone(), items[1].clone())?;
    Ok((old, NativeCtl::intent_replace.gas_of()))
});

intent_std_fn!(call_intent_destroy, |exec, bindings, intent_state, intents, argv| {
    ctl_require_edit(exec, "intent_destroy")?;
    ctl_expect_no_arg(argv, "intent_destroy")?;
    let (owner, id) = owned_bound_intent(bindings, intent_state, intents, "intent_destroy")?;
    intents.destroy(&owner, id)?;
    Ok((Value::Nil, NativeCtl::intent_destroy.gas_of()))
});

intent_std_fn!(call_intent_put_if_absent, |exec, bindings, intent_state, intents, argv| {
    ctl_require_edit(exec, "intent_put_if_absent")?;
    let items = ctl_expect_tuple(argv, "intent_put_if_absent", 2)?;
    let (owner, id) = owned_bound_intent(bindings, intent_state, intents, "intent_put_if_absent")?;
    Ok((
        Value::Bool(intents.put_if_absent(&owner, id, items[0].clone(), items[1].clone())?),
        NativeCtl::intent_put_if_absent.gas_of(),
    ))
});

intent_std_fn!(call_intent_replace_if, |exec, bindings, intent_state, intents, argv| {
    ctl_require_edit(exec, "intent_replace_if")?;
    let items = ctl_expect_tuple(argv, "intent_replace_if", 3)?;
    let (owner, id) = owned_bound_intent(bindings, intent_state, intents, "intent_replace_if")?;
    Ok((
        Value::Bool(intents.replace_if(
            &owner,
            id,
            items[0].clone(),
            items[1].clone(),
            items[2].clone(),
        )?),
        NativeCtl::intent_replace_if.gas_of(),
    ))
});

intent_std_fn!(call_intent_append, |exec, bindings, intent_state, intents, argv| {
    ctl_require_edit(exec, "intent_append")?;
    let items = ctl_expect_tuple(argv, "intent_append", 2)?;
    let (owner, id) = owned_bound_intent(bindings, intent_state, intents, "intent_append")?;
    Ok((
        Value::U64(intents.append(&owner, id, items[0].clone(), &items[1])? as u64),
        NativeCtl::intent_append.gas_of(),
    ))
});

intent_std_fn!(call_intent_inc, |exec, bindings, intent_state, intents, argv| {
    ctl_require_edit(exec, "intent_inc")?;
    let items = ctl_expect_tuple(argv, "intent_inc", 2)?;
    let (owner, id) = owned_bound_intent(bindings, intent_state, intents, "intent_inc")?;
    Ok((
        intents.inc(&owner, id, items[0].clone(), items[1].clone())?,
        NativeCtl::intent_inc.gas_of(),
    ))
});

intent_std_fn!(call_intent_del_if, |exec, bindings, intent_state, intents, argv| {
    ctl_require_edit(exec, "intent_del_if")?;
    let items = ctl_expect_tuple(argv, "intent_del_if", 2)?;
    let (owner, id) = owned_bound_intent(bindings, intent_state, intents, "intent_del_if")?;
    Ok((
        Value::Bool(intents.del_if(&owner, id, items[0].clone(), items[1].clone())?),
        NativeCtl::intent_del_if.gas_of(),
    ))
});

intent_std_fn!(call_intent_take_if, |exec, bindings, intent_state, intents, argv| {
    ctl_require_edit(exec, "intent_take_if")?;
    let items = ctl_expect_tuple(argv, "intent_take_if", 2)?;
    let (owner, id) = owned_bound_intent(bindings, intent_state, intents, "intent_take_if")?;
    let (hit, val) = intents.take_if(&owner, id, items[0].clone(), items[1].clone())?;
    Ok((
        Value::Tuple(TupleItem::new(vec![Value::Bool(hit), val])?),
        NativeCtl::intent_take_if.gas_of(),
    ))
});

intent_std_fn!(call_intent_is_own_handle, |exec, bindings, _intent_state, intents, argv| {
    ctl_require_non_pure(exec, "intent_is_own_handle")?;
    let owner = ctl_contract_owner(bindings, "intent_is_own_handle")?;
    let id = extract_intent_handle_id(&argv, "intent_is_own_handle")?;
    if !intents.exists(id) {
        return Ok((Value::Bool(false), NativeCtl::intent_is_own_handle.gas_of()));
    }
    Ok((
        Value::Bool(intents.is_owner(&owner, id)?),
        NativeCtl::intent_is_own_handle.gas_of(),
    ))
});

intent_std_fn!(call_intent_destroy_if_empty, |exec, bindings, intent_state, intents, argv| {
    ctl_require_edit(exec, "intent_destroy_if_empty")?;
    ctl_expect_no_arg(argv, "intent_destroy_if_empty")?;
    let (owner, id) = owned_bound_intent(bindings, intent_state, intents, "intent_destroy_if_empty")?;
    Ok((
        Value::Bool(intents.destroy_if_empty(&owner, id)?),
        NativeCtl::intent_destroy_if_empty.gas_of(),
    ))
});

intent_std_fn!(call_intent_keys_page, |exec, bindings, intent_state, intents, argv| {
    ctl_require_non_pure(exec, "intent_keys_page")?;
    let items = ctl_expect_tuple(argv, "intent_keys_page", 2)?;
    let (owner, id) = owned_bound_intent(bindings, intent_state, intents, "intent_keys_page")?;
    let cursor = ctl_expect_u32(&items[0], "intent_keys_page", "cursor")? as usize;
    let limit = ctl_expect_u32(&items[1], "intent_keys_page", "limit")? as usize;
    let (next, keys) = intents.keys_page(&owner, id, cursor, limit)?;
    let list = Value::Compo(CompoItem::list(
        keys.into_iter().map(Value::Bytes).collect::<VecDeque<_>>(),
    )?);
    let next_cursor = match next {
        Some(v) => Value::U32(v as u32),
        None => Value::Nil,
    };
    Ok((
        Value::Tuple(TupleItem::new(vec![next_cursor, list])?),
        NativeCtl::intent_keys_page.gas_of(),
    ))
});

intent_std_fn!(call_intent_keys_after, |exec, bindings, intent_state, intents, argv| {
    ctl_require_non_pure(exec, "intent_keys_after")?;
    let items = ctl_expect_tuple(argv, "intent_keys_after", 2)?;
    let (owner, id) = owned_bound_intent(bindings, intent_state, intents, "intent_keys_after")?;
    let limit = ctl_expect_u32(&items[1], "intent_keys_after", "limit")? as usize;
    let start = if items[0].is_nil() {
        None
    } else if let Value::Bytes(_) = &items[0] {
        Some(&items[0])
    } else {
        return itr_err_fmt!(
            ItrErrCode::IntentError,
            "native ctl 'intent_keys_after' requires after_key bytes argument"
        );
    };
    let (next, keys) = intents.keys_after(&owner, id, start, limit)?;
    let list = Value::Compo(CompoItem::list(
        keys.into_iter().map(Value::Bytes).collect::<VecDeque<_>>(),
    )?);
    let next_key = match next {
        Some(v) => Value::Bytes(v),
        None => Value::Nil,
    };
    Ok((
        Value::Tuple(TupleItem::new(vec![next_key, list])?),
        NativeCtl::intent_keys_after.gas_of(),
    ))
});

intent_std_fn!(call_intent_require_absent, |exec, bindings, intent_state, intents, argv| {
    ctl_require_non_pure(exec, "intent_require_absent")?;
    let (owner, id) = owned_bound_intent(bindings, intent_state, intents, "intent_require_absent")?;
    intents.require_absent(&owner, id, &argv)?;
    Ok((Value::Nil, NativeCtl::intent_require_absent.gas_of()))
});

intent_std_fn!(call_intent_require_many, |exec, bindings, intent_state, intents, argv| {
    ctl_require_non_pure(exec, "intent_require_many")?;
    let keys = ctl_expect_list(argv, "intent_require_many")?;
    let (owner, id) = owned_bound_intent(bindings, intent_state, intents, "intent_require_many")?;
    let vals = intents
        .require_many(&owner, id, &keys)?
        .into_iter()
        .collect::<VecDeque<_>>();
    Ok((
        Value::Compo(CompoItem::list(vals)?),
        NativeCtl::intent_require_many.gas_of(),
    ))
});

intent_std_fn!(call_intent_require_map, |exec, bindings, intent_state, intents, argv| {
    ctl_require_non_pure(exec, "intent_require_map")?;
    let keys = ctl_expect_list(argv, "intent_require_map")?;
    let (owner, id) = owned_bound_intent(bindings, intent_state, intents, "intent_require_map")?;
    Ok((
        Value::Compo(CompoItem::map(intents.require_map(&owner, id, &keys)?)?),
        NativeCtl::intent_require_map.gas_of(),
    ))
});

intent_std_fn!(call_intent_put_flat_kv, |exec, bindings, intent_state, intents, argv| {
    ctl_put_kv_list(exec, bindings, intent_state, intents, argv, "intent_put_flat_kv")?;
    Ok((Value::Nil, NativeCtl::intent_put_flat_kv.gas_of()))
});

intent_std_fn!(call_intent_rename, |exec, bindings, intent_state, intents, argv| {
    ctl_require_edit(exec, "intent_rename")?;
    let items = ctl_expect_tuple(argv, "intent_rename", 2)?;
    let (owner, id) = owned_bound_intent(bindings, intent_state, intents, "intent_rename")?;
    intents.move_key(&owner, id, items[0].clone(), items[1].clone())?;
    Ok((Value::Nil, NativeCtl::intent_rename.gas_of()))
});

intent_std_fn!(call_intent_add, |exec, bindings, intent_state, intents, argv| {
    ctl_require_edit(exec, "intent_add")?;
    let items = ctl_expect_tuple(argv, "intent_add", 2)?;
    let (owner, id) = owned_bound_intent(bindings, intent_state, intents, "intent_add")?;
    Ok((
        intents.add(&owner, id, items[0].clone(), items[1].clone())?,
        NativeCtl::intent_add.gas_of(),
    ))
});

intent_std_fn!(call_intent_sub, |exec, bindings, intent_state, intents, argv| {
    ctl_require_edit(exec, "intent_sub")?;
    let items = ctl_expect_tuple(argv, "intent_sub", 2)?;
    let (owner, id) = owned_bound_intent(bindings, intent_state, intents, "intent_sub")?;
    Ok((
        intents.sub(&owner, id, items[0].clone(), items[1].clone())?,
        NativeCtl::intent_sub.gas_of(),
    ))
});

intent_std_fn!(call_intent_take_many, |exec, bindings, intent_state, intents, argv| {
    ctl_require_edit(exec, "intent_take_many")?;
    let keys = ctl_expect_list(argv, "intent_take_many")?;
    let (owner, id) = owned_bound_intent(bindings, intent_state, intents, "intent_take_many")?;
    let vals = intents
        .take_many(&owner, id, &keys)?
        .into_iter()
        .collect::<VecDeque<_>>();
    Ok((
        Value::Compo(CompoItem::list(vals)?),
        NativeCtl::intent_take_many.gas_of(),
    ))
});

intent_std_fn!(call_intent_take_map, |exec, bindings, intent_state, intents, argv| {
    ctl_require_edit(exec, "intent_take_map")?;
    let keys = ctl_expect_list(argv, "intent_take_map")?;
    let (owner, id) = owned_bound_intent(bindings, intent_state, intents, "intent_take_map")?;
    Ok((
        Value::Compo(CompoItem::map(intents.take_map(&owner, id, &keys)?)?),
        NativeCtl::intent_take_map.gas_of(),
    ))
});

intent_std_fn!(call_intent_del_many, |exec, bindings, intent_state, intents, argv| {
    ctl_require_edit(exec, "intent_del_many")?;
    let keys = ctl_expect_list(argv, "intent_del_many")?;
    let (owner, id) = owned_bound_intent(bindings, intent_state, intents, "intent_del_many")?;
    Ok((
        Value::U64(intents.del_many(&owner, id, &keys)? as u64),
        NativeCtl::intent_del_many.gas_of(),
    ))
});

intent_std_fn!(call_intent_has_all, |exec, bindings, intent_state, intents, argv| {
    ctl_require_non_pure(exec, "intent_has_all")?;
    let keys = ctl_expect_list(argv, "intent_has_all")?;
    let (owner, id) = owned_bound_intent(bindings, intent_state, intents, "intent_has_all")?;
    Ok((
        Value::Bool(intents.has_all(&owner, id, &keys)?),
        NativeCtl::intent_has_all.gas_of(),
    ))
});

intent_std_fn!(call_intent_has_any, |exec, bindings, intent_state, intents, argv| {
    ctl_require_non_pure(exec, "intent_has_any")?;
    let keys = ctl_expect_list(argv, "intent_has_any")?;
    let (owner, id) = owned_bound_intent(bindings, intent_state, intents, "intent_has_any")?;
    Ok((
        Value::Bool(intents.has_any(&owner, id, &keys)?),
        NativeCtl::intent_has_any.gas_of(),
    ))
});

intent_std_fn!(call_intent_consume, |exec, bindings, intent_state, intents, argv| {
    ctl_require_edit(exec, "intent_consume")?;
    let (owner, id) = owned_bound_intent(bindings, intent_state, intents, "intent_consume")?;
    Ok((
        intents.consume(&owner, id, &argv)?,
        NativeCtl::intent_consume.gas_of(),
    ))
});

intent_std_fn!(call_intent_consume_many, |exec, bindings, intent_state, intents, argv| {
    ctl_require_edit(exec, "intent_consume_many")?;
    let keys = ctl_expect_list(argv, "intent_consume_many")?;
    let (owner, id) = owned_bound_intent(bindings, intent_state, intents, "intent_consume_many")?;
    let vals = intents
        .consume_many(&owner, id, &keys)?
        .into_iter()
        .collect::<VecDeque<_>>();
    Ok((
        Value::Compo(CompoItem::list(vals)?),
        NativeCtl::intent_consume_many.gas_of(),
    ))
});

intent_std_fn!(call_intent_put_if_absent_or_match, |exec, bindings, intent_state, intents, argv| {
    ctl_require_edit(exec, "intent_put_if_absent_or_match")?;
    let items = ctl_expect_tuple(argv, "intent_put_if_absent_or_match", 2)?;
    let (owner, id) = owned_bound_intent(bindings, intent_state, intents, "intent_put_if_absent_or_match")?;
    Ok((
        Value::Bool(intents.put_if_absent_or_match(
            &owner,
            id,
            items[0].clone(),
            items[1].clone(),
        )?),
        NativeCtl::intent_put_if_absent_or_match.gas_of(),
    ))
});
