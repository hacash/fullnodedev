

fn check_call_mode(mode: ExecMode, inst: Bytecode, in_callcode: bool) -> VmrtErr {
    use ExecMode::*;
    use Bytecode::*;
    if in_callcode {
        // In CALLCODE execution, no further call instructions are allowed.
        return itr_err_code!(CallInCallcode)
    }
    macro_rules! not_ist {
        ( $( $ist: expr ),+ ) => {
            ![$( $ist ),+].contains(&inst)
        }
    }
    match mode {
        Main if not_ist!(CALL, CALLVIEW, CALLPURE, CALLCODE) => itr_err_code!(CallOtherInMain),
        P2sh if not_ist!(CALLVIEW, CALLPURE, CALLCODE) => itr_err_code!(CallOtherInP2sh),
        // Abst intentionally allows this/self/super: root frame keeps state_addr as the
        // concrete contract address passed by VM entry, while code_owner may come from
        // inherited abstract function dispatch.
        Abst if not_ist!(CALLTHIS, CALLSELF, CALLSUPER, CALLVIEW, CALLPURE, CALLCODE) => itr_err_code!(CallInAbst),
        View if not_ist!(CALLVIEW, CALLPURE) => itr_err_code!(CallLocInView),
        Pure if not_ist!(CALLPURE) => itr_err_code!(CallInPure),
        // Outer and Inner allow all call instructions.
        // Guard-false arms for Main/P2sh/Abst/View/Pure also fall here (call is allowed).
        Main | P2sh | Abst | Outer | Inner | View | Pure => Ok(()),
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


fn unpack_list(
    i: u8,
    locals: &mut Stack,
    list: &VecDeque<Value>,
    gst: &GasExtra,
) -> VmrtRes<i64> {
    let start = i as usize;
    if locals.len() < start + list.len() {
        return itr_err_code!(OutOfStack)
    }
    let mut gas = 0i64;
    for (off, item) in list.iter().enumerate() {
        let v = item.clone();
        gas += gst.stack_write(v.val_size());
        *locals.edit((start + off) as u8)? = v;
    }
    Ok(gas)
}
