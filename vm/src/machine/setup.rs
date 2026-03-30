fn ensure_vm_run_ready(ctx: &dyn Context) -> Rerr {
    const TY3: u8 = TransactionType3::TYPE;
    let txty = ctx.env().tx.ty;
    if txty < TY3 {
        return errf!(
            "current transaction type {} too low to setup vm, requires at least {}",
            txty,
            TY3
        );
    }
    Ok(())
}

pub fn setup_vm_runtime_gascap(ctx: &mut dyn Context, height: u64) -> (GasExtra, SpaceCap) {
    let Some(conf) = ctx.vm_runtime_config() else {
        return (GasExtra::new(height), SpaceCap::new(height));
    };
    let Ok(conf) = conf.downcast::<(GasExtra, SpaceCap)>() else {
        return (GasExtra::new(height), SpaceCap::new(height));
    };
    *conf
}

/// Falsy return => success. Non-falsy or object return => recoverable (XError::revert). Runtime-only values crossing the VM boundary are unrecoverable (XError::fault).
pub fn check_vm_return_value(rv: &Value, err_msg: &str) -> XRerr {
    rv.check_vm_boundary_retv()
        .map_err(|e| XError::fault(format!("{} return cannot cross VM boundary: {}", err_msg, e)))?;
    use Value::*;
    let detail: Option<String> = match rv {
        Nil => None,
        Bool(false) => None,
        Bool(true) => Some("code 1".to_owned()),
        U8(n) => (*n != 0).then(|| format!("code {}", n)),
        U16(n) => (*n != 0).then(|| format!("code {}", n)),
        U32(n) => (*n != 0).then(|| format!("code {}", n)),
        U64(n) => (*n != 0).then(|| format!("code {}", n)),
        U128(n) => (*n != 0).then(|| format!("code {}", n)),
        Bytes(buf) => maybe!(
            buf_is_empty_or_all_zero(buf),
            None,
            Some(match ascii_show_string(buf) {
                Some(s) => format!("bytes {:?}", s),
                None => format!("bytes 0x{}", buf.to_hex()),
            })
        ),
        Value::Address(a) => maybe!(
            buf_is_empty_or_all_zero(a.as_bytes()),
            None,
            Some(format!("address {}", a.to_readable()))
        ),
        HeapSlice(_) | Handle(_) => never!(),
        Tuple(_) | Compo(_) => Some(format!("object {}", rv.to_json())),
    };
    match detail {
        None => Ok(()),
        Some(d) => Err(XError::revert(format!("{} return error {}", err_msg, d))),
    }
}


/*****************************************************/
pub fn setup_vm_run_main(
    ctx: &mut dyn Context,
    codeconf: u8,
    payload: Vec<u8>,
) -> Ret<(GasUse, Value)> {
    // Bytecode verification is intentionally handled by upper-layer action validators before calling setup_vm_run_main.
    ensure_vm_run_ready(ctx)?;
    let (cost, rv) = ctx.vm_call(Box::new(VmEntryReq::Main {
        code_type: CodeConf::parse(codeconf)?.code_type(),
        codes: Arc::from(payload),
    }))?;
    let Ok(rv) = rv.downcast::<Value>() else {
        return errf!("vm call return type mismatch");
    };
    Ok((cost, *rv))
}

pub fn setup_vm_run_p2sh(
    ctx: &mut dyn Context,
    codeconf: u8,
    payload: Vec<u8>,
    param: Value,
) -> Ret<(GasUse, Value)> {
    // Lock script verification is intentionally handled by upper-layer action validators before calling setup_vm_run_p2sh.
    ensure_vm_run_ready(ctx)?;
    let payload = ByteView::from_arc(Arc::from(payload));
    let payload_ref = payload.as_slice();
    let (state_addr, mv1) = Address::create(payload_ref)?;
    let (libs, mv2) = ContractAddressW1::create(&payload_ref[mv1..])?;
    let mv = mv1 + mv2;
    let intent_binding = ctx.vm_current_intent_scope();
    let (cost, rv) = ctx.vm_call(Box::new(VmEntryReq::P2sh {
        code_type: CodeConf::parse(codeconf)?.code_type(),
        state_addr,
        libs: libs.into_list(),
        codes: payload.slice(mv, payload.len())?,
        intent_binding,
        param,
    }))?;
    let Ok(rv) = rv.downcast::<Value>() else {
        return errf!("vm call return type mismatch");
    };
    Ok((cost, *rv))
}

pub fn setup_vm_run_abst(
    ctx: &mut dyn Context,
    target: AbstCall,
    payload: Address,
    param: Value,
) -> Ret<(GasUse, Value)> {
    ensure_vm_run_ready(ctx)?;
    let intent_binding = ctx.vm_current_intent_scope();
    let (cost, rv) = ctx.vm_call(Box::new(VmEntryReq::Abst {
        kind: target,
        contract_addr: ContractAddress::from_addr(payload)?,
        intent_binding,
        param,
    }))?;
    let Ok(rv) = rv.downcast::<Value>() else {
        return errf!("vm call return type mismatch");
    };
    Ok((cost, *rv))
}








/*****************************************/






#[cfg(test)]
mod setup_tests {
    use super::*;

    #[test]
    fn check_vm_return_value_faults_handle_inside_tuple() {
        let rv = Value::Tuple(
            TupleItem::new(vec![Value::U8(1), Value::handle(7u32)]).unwrap(),
        );
        let err = check_vm_return_value(&rv, "main call").unwrap_err();
        assert!(err.is_fault());
        assert!(err.to_string().contains("cannot cross VM boundary"));
    }
}
