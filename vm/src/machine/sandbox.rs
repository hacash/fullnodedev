

/* return gasuse, retval */
// Test/tooling helper only; non-production execution path.
pub fn sandbox_call(ctx: &mut dyn Context, contract: ContractAddress, funcname: String, params: &str) -> Ret<(i64, String)> {
    use rt::Bytecode::*;
    use rt::verify_bytecodes;

    let hei = ctx.env().block.height;

    let mainaddr = ctx.env().tx.main.clone();
    let txinfo = &ctx.env().tx as *const TxInfo;
    let txinfo = txinfo as *mut TxInfo;
    unsafe {
        (*txinfo).swap_addrs(&mut vec![mainaddr, contract.into_addr()]);
    }

    let gascp = GasExtra::new(hei);
    let gas_limit = gascp.max_gas_of_tx;
    let gas = &mut gas_limit.clone();

    let mut codes: Vec<u8> = vec![];
    parse_push_params(&mut codes, params)?;

    // call contract
    let fnsg = calc_func_sign(&funcname);
    codes.push(CALLEXT as u8);
    codes.push(1); // lib idx
    codes.append(&mut fnsg.to_vec());
    codes.push(RET as u8); // return the value
    verify_bytecodes(&codes)?;

    // do call
    // Intentionally do not restore level: sandbox call is one-shot and its context
    // state is discarded by the caller after return.
    ctx.level_set(ACTION_CTX_LEVEL_CALL_MAIN);
    let mut exenv = ExecEnv{ ctx, gas };
    let mut vmb = global_machine_manager().assign(hei);
    let res = vmb.machine.as_mut().unwrap().main_call_raw(&mut exenv, CodeType::Bytecode, codes.into());
    res.map(|v|(
        gas_limit-*gas, v.to_debug_json()
    ))

}



fn parse_push_params(codes: &mut Vec<u8>, pms: &str) -> Rerr {
    macro_rules! push { ( $( $a: expr ),+) => { $( codes.push($a as u8) );+ } }
    use Bytecode::*;
    let mut pms_count = 0usize;
    for part in pms.split(',').map(str::trim).filter(|s| !s.is_empty()) {
        let (v, t) = match part.split_once(':') {
            Some((v, t)) => (v.trim(), t.trim()),
            None => (part, "nil"),
        };
        parse_one_param(codes, t, v)?;
        pms_count += 1;
    }
    match pms_count {
        0      => { push!(PNIL); } // none argv
        1      => { /* single param: push raw value; contract uses PUT 0 ROLL0, not UNPACK */ }
        2..255 => { push!(PU8, pms_count, PACKARGS); }
        255..  => return errf!("param number is too much"),
    }
    Ok(())
}


fn parse_one_param(codes: &mut Vec<u8>, t: &str, v: &str) -> Rerr {
    use Bytecode::*;
    use ValueTy::*;
    macro_rules! push { ( $( $a: expr ),+) => { $( codes.push($a as u8) );+ } }
    let ty = ValueTy::from_name(t).map_err(|_| format!("unsupported param type '{}'", t))?;
    match ty {
        Nil  => push!(PNIL),
        Bool => match v {
            "true" => push!(PTRUE),
            "false" => push!(PFALSE),
            _ => return errf!("invalid bool argument '{}'", v),
        },
        U8   => {
            let n = v.parse::<u8>().map_err(|e| format!("invalid u8 argument '{}': {}", v, e))?;
            push!(PU8, n);
        },
        U16   => {
            let n = v
                .parse::<u16>()
                .map_err(|e| format!("invalid u16 argument '{}': {}", v, e))?;
            push!(PU16);
            codes.append(&mut Vec::from(n.to_be_bytes()));
        },
        U32   => {
            let n = v
                .parse::<u32>()
                .map_err(|e| format!("invalid u32 argument '{}': {}", v, e))?;
            push!(PBUF, 4);
            codes.append(&mut Vec::from(n.to_be_bytes()));
            push!(CU32);
        },
        U64   => {
            let n = v
                .parse::<u64>()
                .map_err(|e| format!("invalid u64 argument '{}': {}", v, e))?;
            push!(PBUF, 8);
            codes.append(&mut Vec::from(n.to_be_bytes()));
            push!(CU64);
        },
        U128   => {
            let n = v
                .parse::<u128>()
                .map_err(|e| format!("invalid u128 argument '{}': {}", v, e))?;
            push!(PBUF, 16);
            codes.append(&mut Vec::from(n.to_be_bytes()));
            push!(CU128);
        },
        Address => {
            let adr = field::Address::from_readable(v)
                .map_err(|e| format!("invalid address argument '{}': {}", v, e))?;
            push!(PBUF, field::Address::SIZE);
            codes.append(&mut adr.into_vec());
            push!(CTO, ty);
        },
        Bytes => {
            let hex_body = v.strip_prefix("0x").unwrap_or(v);
            let mut bts = hex::decode(hex_body)
                .map_err(|e| format!("invalid bytes argument '{}': {}", v, e))?;
            push!(PBUF, bts.len());
            codes.append(&mut bts);
        },
        _ => return errf!("unsupported param type '{}'", t)
    };
    Ok(())
}

#[cfg(test)]
mod sandbox_parse_tests {
    use super::*;

    #[test]
    fn parse_push_params_accepts_bytes_with_0x_prefix() {
        let mut codes = vec![];
        parse_push_params(&mut codes, "0x57495657414b:bytes,0:u16").unwrap();
        assert!(!codes.is_empty());
    }

    #[test]
    fn parse_push_params_reports_invalid_bytes() {
        let mut codes = vec![];
        let err = parse_push_params(&mut codes, "0xzz:bytes").unwrap_err();
        assert!(err.to_string().contains("invalid bytes argument"));
    }
}
