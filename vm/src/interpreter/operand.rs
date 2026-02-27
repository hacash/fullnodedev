

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
    let opt = mark >> 6; // 0b00000011
    let idx = mark & 0b00111111; // max=64
    let basev = locals.edit(idx)?;
    match opt {
        0 => locop_arithmetic(basev, &mut value, add_checked), // +=
        1 => locop_arithmetic(basev, &mut value, sub_checked), // -=
        2 => locop_arithmetic(basev, &mut value, mul_checked), // *=
        3 => locop_arithmetic(basev, &mut value, div_checked), // /=
        _ => unreachable!(), // return itr_err_fmt!(InstParamsErr, "local operand {} not find", a)
    }?;
    Ok(())
}


fn local_logic(mark: u8, locals: &mut Stack, value: &mut Value) -> VmrtErr {
    let opt = mark >> 5; // 0b00000111
    let idx = mark & 0b00011111; // max=32
    let basev = locals.edit(idx)?;
    match opt {
        0 => locop_btw(value, basev, lgc_and),
        1 => locop_btw(value, basev, lgc_or),
        2 => locop_btw(value, basev, lgc_equal),
        3 => locop_btw(value, basev, lgc_not_equal),
        4 => locop_btw(value, basev, lgc_less),
        5 => locop_btw(value, basev, lgc_less_equal),
        6 => locop_btw(value, basev, lgc_greater),
        7 => locop_btw(value, basev, lgc_greater_equal),
        _ => unreachable!(), // return itr_err_fmt!(InstParamsErr, "local operand {} not find", a)
    }?;
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
