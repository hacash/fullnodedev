

type IRNRef<'a> = &'a Box<dyn IRNode>;

// (u16::MAX/2 - jpl as u16).into();
const JMP_INST_LEN: usize = 3; //u8 + u16
const BLOCK_CODES_MAX_LEN: usize = i16::MAX as usize - JMP_INST_LEN - 64;

fn is_stmt_block(n: IRNRef) -> bool {
    if let Some(arr) = n.as_any().downcast_ref::<IRNodeArray>() {
        return arr.inst == Bytecode::IRBLOCK;
    }
    false
}


fn compile_block(inst: Bytecode, list: &Vec<Box<dyn IRNode>>) -> VmrtRes<Vec<u8>> {
    let is_expr = inst == Bytecode::IRBLOCKR;
    if is_expr {
        match list.last() {
            None => return itr_err_fmt!(ComplieError, "block expression cannot be empty"),
            Some(last) if !last.hasretval() => return itr_err_fmt!(ComplieError, "block expression must return value"),
            _ => {},
        }
    }
    let mut codes = Vec::new();
    for (idx, one) in list.iter().enumerate() {
        one.codegen_into(&mut codes)?;
        if one.hasretval() {
            if is_expr && idx + 1 == list.len() {
                continue;
            }
            codes.push(POP as u8);
        }
    }
    Ok(codes)
}

fn compile_list(_inst: Bytecode, list: &Vec<Box<dyn IRNode>>) -> VmrtRes<Vec<u8>> {
    let mut codes = Vec::new();
    for one in list {
        one.codegen_into(&mut codes)?;
    }
    Ok(codes)
}


fn compile_double(btcd: Bytecode, x: IRNRef, y: IRNRef) -> VmrtRes<Option<Vec<u8>>> {
    Ok(match btcd {
        IRWHILE => Some(compile_while(x, y)?),
        _ => None
    })
}


fn compile_while(x: IRNRef, y: IRNRef) -> VmrtRes<Vec<u8>> {
    const JIL: usize = JMP_INST_LEN;
    const MAXL: usize = BLOCK_CODES_MAX_LEN;
    // condition
    let mut cond = Vec::new();
    x.codegen_into(&mut cond)?;
    let mut body = Vec::new();
    y.codegen_into(&mut body)?;
    // IRBLOCK already discards return values of its internal statements.
    // Only append a POP when the body is NOT a statement block container.
    if y.hasretval() && !is_stmt_block(y) {
        body.push(POP as u8); // pop inst
    }
    let body_l = body.len() + JIL;
    let alls_l = body_l + cond.len() + JIL;
    // check code len
    if body_l > MAXL || alls_l > MAXL {
        return itr_err_fmt!(ComplieError, "compile ir codes too long")
    }
    // condition
    let mut codes = Vec::with_capacity(cond.len() + 1 + 2 + body.len() + 1 + 2);
    codes.extend_from_slice(&cond);
    codes.push(BRSLN as u8);
    codes.extend_from_slice(&(body_l as i16).to_be_bytes());
    codes.extend_from_slice(&body);
    codes.push(JMPSL as u8);
    codes.extend_from_slice(&(-(alls_l as i16)).to_be_bytes());
    Ok(codes)
}


/**************************************************/


fn compile_triple(btcd: Bytecode, x: IRNRef, y: IRNRef, z: IRNRef) -> VmrtRes<Option<Vec<u8>>> {
    Ok(match btcd {
        IRIF | IRIFR => Some(compile_if(btcd, x, y, z)?),
        _ => None
    })
}


fn compile_if(btcd: Bytecode, x: IRNRef, y: IRNRef, z: IRNRef) -> VmrtRes<Vec<u8>> {
    const JIL: usize = JMP_INST_LEN;
    const MAXL: usize = BLOCK_CODES_MAX_LEN;
    let mut cond = Vec::new();
    x.codegen_into(&mut cond)?;
    let mut if_br = Vec::new();
    y.codegen_into(&mut if_br)?;
    let is_expr = btcd == Bytecode::IRIFR;
    if is_expr && !y.hasretval() {
        return itr_err_fmt!(ComplieError, "if expression branch must return value");
    }
    // IRBLOCK already discards return values internally.
    if !is_expr && y.hasretval() && !is_stmt_block(y) {
        if_br.push(POP as u8); // pop inst
    }
    let mut el_br = Vec::new();
    z.codegen_into(&mut el_br)?;
    if is_expr && !z.hasretval() {
        return itr_err_fmt!(ComplieError, "if expression branch must return value");
    }
    if !is_expr && z.hasretval() && !is_stmt_block(z) {
        el_br.push(POP as u8); // pop inst
    }
    let if_l = if_br.len();
    let el_l = el_br.len() + JIL;
    // check code len
    if if_l > MAXL || el_l > MAXL {
        return itr_err_fmt!(ComplieError, "compile ir codes too long")
    }
    let mut codes = Vec::with_capacity(
        cond.len() + 1 + 2 + el_br.len() + 1 + 2 + if_br.len()
    );
    codes.extend_from_slice(&cond);
    codes.push(BRSL as u8);
    codes.extend_from_slice(&(el_l as i16).to_be_bytes());
    codes.extend_from_slice(&el_br);
    codes.push(JMPSL as u8);
    codes.extend_from_slice(&(if_l as i16).to_be_bytes());
    codes.extend_from_slice(&if_br);
    Ok(codes)
}
