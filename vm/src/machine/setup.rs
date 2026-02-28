

/* return gas, val */
/// Call level from exec mode: Main/P2sh → CALL_MAIN, Abst → CALL_CONTRACT
fn call_level_from_exec_mode(ty: u8) -> Ret<usize> {
    use crate::rt::*;
    use crate::rt::ExecMode::*;
    let mode = ExecMode::try_from_u8(ty).map_err(|e| e.to_string())?;
    match mode {
        Main | P2sh => Ok(ACTION_CTX_LEVEL_CALL_MAIN),
        Abst        => Ok(ACTION_CTX_LEVEL_CALL_CONTRACT),
        _ => errf!("unknown exec mode {}", ty),
    }
}

/// Check VM return value using unified truthiness:
/// falsy (`canbe_bool == false`) is success, otherwise execution failure.
fn vm_error_detail(rv: &Value) -> String {
    match rv {
        Value::Nil => "code 0".to_owned(),
        Value::Bool(b) => format!("code {}", maybe!(b, 1, 0)),
        Value::U8(n) => format!("code {}", n),
        Value::U16(n) => format!("code {}", n),
        Value::U32(n) => format!("code {}", n),
        Value::U64(n) => format!("code {}", n),
        Value::U128(n) => format!("code {}", n),
        Value::Bytes(buf) => match ascii_show_string(buf) {
            Some(s) => format!("bytes {:?}", s),
            None => format!("bytes 0x{}", buf.to_hex()),
        },
        Value::Address(a) => format!("address {}", a.to_readable()),
        _ => "error <object>".to_owned(),
    }
}

pub fn check_vm_return_value(rv: &Value, err_msg: &str) -> Rerr {
    match rv.canbe_bool() {
        Ok(false) => Ok(()),
        Ok(true) | Err(_) => erruf!("{} return error {}", err_msg, vm_error_detail(rv)),
    }
}

pub fn setup_vm_run(ctx: &mut dyn Context, ty: u8, mk: u8, cd: std::sync::Arc<[u8]>, pm: Value) -> Ret<(i64, Value)> {
    // Bytecode verification is intentionally handled by upper-layer action validators before calling setup_vm_run.
    // check tx type
    const TY3: u8 = TransactionType3::TYPE;
    let txty = ctx.env().tx.ty;
    if txty < TY3 {
        return errf!("current transaction type {} too low to setup vm, need at least {}", txty, TY3)
    }
    // Ensure VM is initialized if a VM assigner is registered. Protocol normally does this at tx execution entry, but callers may invoke `setup_vm_run` directly in tests/tools.
    protocol::setup::do_vm_init(ctx)?;
    if ctx.vm().is_nil() {
        let gmx = ctx.tx().fee_extend().unwrap_or(0);
        return errf!("vm not initialized for this tx (tx_type={}, gas_max_byte={})", txty, gmx)
    }
    // Set ctx.level for this VM call and restore it after returning.
    let old_level = ctx.level();
    ctx.level_set(call_level_from_exec_mode(ty)?);

    // IMPORTANT: VM execution is re-entrant through EXTACTION -> action.execute() -> setup_vm_run(). We must keep the same VM instance visible via `ctx.vm()` during the whole tx/call chain; otherwise nested setup_vm_run() would allocate a new VM and then be silently overwritten. To avoid Rust borrow aliasing (`&mut ctx` + `&mut ctx.vm()`), we perform a *single* localized raw-pointer call here, keeping the VM in-place inside Context. Safety assumptions (consensus-critical): - Single-threaded execution. - No code path replaces `ctx.vm` while `VM::call` is running (only `setup_vm_run` does setup). - The VM implementation may re-enter `setup_vm_run` via EXTACTION, and that re-entry must observe the same VM instance to preserve gas accounting and call-stack invariants.
    let ctxptr = ctx as *mut dyn Context;
    let res = unsafe {
        let vm = (*ctxptr).vm() as *mut dyn VM;
        (*vm).call(VMCall::new(&mut *ctxptr, ty, mk, cd, Box::new(pm))).into_ret()
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
