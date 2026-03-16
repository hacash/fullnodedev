fn with_vm_call_level<T>(
    ctx: &mut dyn Context,
    entry: EntryKind,
    f: impl FnOnce(&mut dyn Context) -> T,
) -> T {
    let old_level = ctx.level();
    ctx.level_set(entry.action_level());
    let res = f(ctx);
    ctx.level_set(old_level);
    res
}

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

fn setup_vm_run_entry(
    ctx: &mut dyn Context,
    entry: EntryKind,
    target: u8,
    payload: std::sync::Arc<[u8]>,
    param: Value,
) -> Ret<(i64, Value)> {
    ensure_vm_run_ready(ctx)?;
    let res = with_vm_call_level(ctx, entry, |ctx| {
        ctx.vm_call(entry as u8, target, payload, Box::new(param))
            .into_tret()
    });
    let (cost, rv) = res?;
    Ok((cost, Value::bytes(rv)))
}

/// Falsy return => success. Non-falsy or object return => recoverable (XError::revert). HeapSlice => unrecoverable (XError::fault).
pub fn check_vm_return_value(rv: &Value, err_msg: &str) -> XRerr {
    use Value::*;
    let detail: Option<String> = match rv {
        Nil => None,
        Bool(false) => None,
        Bool(true) => Some("code 1".to_owned()),
        U8(n)   => (*n != 0).then(|| format!("code {}", n)),
        U16(n)  => (*n != 0).then(|| format!("code {}", n)),
        U32(n)  => (*n != 0).then(|| format!("code {}", n)),
        U64(n)  => (*n != 0).then(|| format!("code {}", n)),
        U128(n) => (*n != 0).then(|| format!("code {}", n)),
        Bytes(buf) => maybe!(buf_is_empty_or_all_zero(buf), None, Some(match ascii_show_string(buf) {
            Some(s) => format!("bytes {:?}", s),
            None => format!("bytes 0x{}", buf.to_hex()),
        })),
        Value::Address(a) => maybe!(buf_is_empty_or_all_zero(a.as_bytes()), None, 
            Some(format!("address {}", a.to_readable()))
        ),
        HeapSlice(_) => return Err(XError::fault(format!("{} return type HeapSlice is not supported", err_msg))),
        Tuple(_) | Compo(_) => Some(format!("object {}", rv.to_json())),
    };
    match detail {
        None => Ok(()),
        Some(d) => Err(XError::revert(format!("{} return error {}", err_msg, d))),
    }
}

pub fn setup_vm_run_main(
    ctx: &mut dyn Context,
    codeconf: u8,
    payload: std::sync::Arc<[u8]>,
) -> Ret<(i64, Value)> {
    // Bytecode verification is intentionally handled by upper-layer action validators before calling setup_vm_run_main.
    setup_vm_run_entry(ctx, EntryKind::Main, codeconf, payload, Value::Nil)
}

pub fn setup_vm_run_p2sh(
    ctx: &mut dyn Context,
    codeconf: u8,
    payload: std::sync::Arc<[u8]>,
    param: Value,
) -> Ret<(i64, Value)> {
    // Lock script verification is intentionally handled by upper-layer action validators before calling setup_vm_run_p2sh.
    setup_vm_run_entry(ctx, EntryKind::P2sh, codeconf, payload, param)
}

pub fn setup_vm_run_abst(
    ctx: &mut dyn Context,
    target: AbstCall,
    payload: std::sync::Arc<[u8]>,
    param: Value,
) -> Ret<(i64, Value)> {
    setup_vm_run_entry(ctx, EntryKind::Abst, target as u8, payload, param)
}
