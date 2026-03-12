

/* return gas, val */
/// Call level from entry kind: Main/P2sh are external root calls, Abst is contract-level.
fn call_level_from_entry_kind(entry: u8) -> Ret<usize> {
    use crate::rt::*;
    let entry = EntryKind::try_from_u8(entry).map_err(|e| e.to_string())?;
    match entry {
        EntryKind::Main | EntryKind::P2sh => Ok(ACTION_CTX_LEVEL_CALL_MAIN),
        EntryKind::Abst => Ok(ACTION_CTX_LEVEL_CALL_CONTRACT),
    }
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
        Args(_) | Compo(_) => Some(format!("object {}", rv.to_json())),
    };
    match detail {
        None => Ok(()),
        Some(d) => Err(XError::revert(format!("{} return error {}", err_msg, d))),
    }
}

pub fn setup_vm_run(ctx: &mut dyn Context, entry: u8, kind: u8, payload: std::sync::Arc<[u8]>, param: Value) -> Ret<(i64, Value)> {
    // Bytecode verification is intentionally handled by upper-layer action validators before calling setup_vm_run.
    // check tx type
    const TY3: u8 = TransactionType3::TYPE;
    let txty = ctx.env().tx.ty;
    if txty < TY3 {
        return errf!("current transaction type {} too low to setup vm, requires at least {}", txty, TY3)
    }
    // Ensure VM is initialized if a VM assigner is registered. Protocol normally does this at tx execution entry, but callers may invoke `setup_vm_run` directly in tests/tools.
    protocol::setup::do_vm_init(ctx)?;
    if ctx.vm().is_nil() {
        let gmx = ctx.tx().fee_extend().unwrap_or(0);
        return errf!("vm not initialized for this tx (tx_type={}, gas_max_byte={})", txty, gmx)
    }
    // Set ctx.level for this VM call and restore it after returning.
    let old_level = ctx.level();
    ctx.level_set(call_level_from_entry_kind(entry)?);

    // IMPORTANT: VM execution is re-entrant through ACTION -> action.execute() -> setup_vm_run(). We must keep the same VM instance visible via `ctx.vm()` during the whole tx/call chain; otherwise nested setup_vm_run() would allocate a new VM and then be silently overwritten. To avoid Rust borrow aliasing (`&mut ctx` + `&mut ctx.vm()`), we perform a *single* localized raw-pointer call here, keeping the VM in-place inside Context. Safety assumptions (consensus-critical): - Single-threaded execution. - No code path replaces `ctx.vm` while `VM::call` is running (only `setup_vm_run` does setup). - The VM implementation may re-enter `setup_vm_run` via ACTION, and that re-entry must observe the same VM instance to preserve gas accounting and call-stack invariants.
    let ctxptr = ctx as *mut dyn Context;
    let res = unsafe {
        let vm = (*ctxptr).vm() as *mut dyn VM;
        (*vm).call(VMCall::new(&mut *ctxptr, entry, kind, payload, Box::new(param))).into_tret()
    };
    ctx.level_set(old_level);
    let (cost, rv) = res?;
    Ok((cost, Value::bytes(rv)))
}

/// VM assign function for protocol layer registration.
/// Called by `do_vm_init` at TX execution entry.
pub fn vm_assign(height: u64) -> Box<dyn VM> {
    Box::new(global_machine_manager().assign(height))
}
