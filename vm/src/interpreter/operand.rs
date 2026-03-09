fn check_call_mode(exec: ExecCtx, call: &CallSpec) -> VmrtErr {
    if call.requires_external_visibility() && exec.entry == EntryKind::Abst {
        return itr_err_code!(CallInAbst);
    }
    if matches!(
        call,
        CallSpec::Invoke {
            effect: EffectMode::Edit,
            ..
        }
    ) && exec.entry == EntryKind::P2sh
    {
        return itr_err_code!(CallOtherInP2sh);
    }
    if let CallSpec::Invoke { target, effect, .. } = *call {
        match (
            exec.is_outer_entry(),
            exec.entry,
            target.switches_context(),
            effect,
        ) {
            (true, EntryKind::Main, false, EffectMode::Edit) => {
                return itr_err_code!(CallOtherInMain)
            }
            _ => {}
        }
    }
    let next = call.next_effect(exec.effect);
    match (exec.effect, next) {
        (EffectMode::Pure, EffectMode::Pure) => Ok(()),
        (EffectMode::Pure, _) => itr_err_code!(CallInPure),
        (EffectMode::View, EffectMode::Edit) => itr_err_code!(CallLocInView),
        (EffectMode::Edit, _) | (EffectMode::View, EffectMode::View | EffectMode::Pure) => Ok(()),
    }
}

fn local_operand(mark: u8, locals: &mut Stack, mut value: Value) -> VmrtErr {
    let (opt, idx) = decode_local_operand_mark(mark);
    let basev = locals.edit(idx)?;
    match opt {
        LxOp::Add => locop_arithmetic(basev, &mut value, add_checked),
        LxOp::Sub => locop_arithmetic(basev, &mut value, sub_checked),
        LxOp::Mul => locop_arithmetic(basev, &mut value, mul_checked),
        LxOp::Div => locop_arithmetic(basev, &mut value, div_checked),
    }?;
    Ok(())
}

fn local_logic(mark: u8, locals: &mut Stack, value: &mut Value) -> VmrtErr {
    let (opt, idx) = decode_local_logic_mark(mark);
    let basev = locals.edit(idx)?;
    let out = match opt {
        LxLg::And => lgc_and(basev, value),
        LxLg::Or => lgc_or(basev, value),
        LxLg::Eq => lgc_equal(basev, value),
        LxLg::Ne => lgc_not_equal(basev, value),
        LxLg::Gt => lgc_greater(basev, value),
        LxLg::Ge => lgc_greater_equal(basev, value),
        LxLg::Lt => lgc_less(basev, value),
        LxLg::Le => lgc_less_equal(basev, value),
    }?;
    *value = out;
    Ok(())
}

fn unpack_seq(i: u8, locals: &mut Stack, items: Vec<Value>, gst: &GasExtra) -> VmrtRes<i64> {
    let start = i as usize;
    if locals.len() < start + items.len() {
        return itr_err_code!(OutOfStack);
    }
    let mut gas = 0i64;
    for (off, v) in items.into_iter().enumerate() {
        gas += gst.stack_write(v.val_size());
        let idx = u8::try_from(start + off).map_err(|_| ItrErr::code(OutOfStack))?;
        *locals.edit(idx)? = v;
    }
    Ok(gas)
}
