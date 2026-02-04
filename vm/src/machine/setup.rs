

/*
    return gas, val
*/
/// Depth from exec mode: 0=entry layer (Main,P2sh, not in contract), 1=in contract (Abst)
pub fn depth_from_exec_mode(ty: u8) -> Ret<i8> {
    use crate::rt::*;
    use crate::rt::ExecMode::*;
    let mode: ExecMode = std_mem_transmute!(ty);
    match mode {
        Main | P2sh => Ok(0),
        Abst        => Ok(1),
        _ => errf!("unknown exec mode {}", ty),
    }
}

/// Check VM return value: only nil or 0 is considered success.
/// Any other value indicates execution failure.
pub fn check_vm_return_value(rv: &Value, err_msg: &str) -> Rerr {
    if rv.check_true() {
        return errf!("{} return error code {}", err_msg, rv.to_uint())
    }
    Ok(())
}

pub fn setup_vm_run(ctx: &mut dyn Context, ty: u8, mk: u8, cd: &[u8], pm: Value) -> Ret<(i64, Value)> {
    // check tx type
    const TY3: u8 = TransactionType3::TYPE;
    let txty = ctx.env().tx.ty;
    if txty < TY3 {
        return errf!("current transaction type {} too low to setup vm, need at least {}", txty, TY3)
    }
    // setup the vm
    if false == ctx.vm().usable() {
        let vmb = global_machine_manager().assign(ctx.env().block.height);
        ctx.vm_replace(Box::new(vmb));
    }
    let depth = depth_from_exec_mode(ty)?;
    // Set ctx.depth (0=Main/P2sh, 1=Abst) before VM call, restored after (even on error)
    let old_depth = ctx.depth().clone();
    ctx.depth_set(CallDepth::new(depth));

    let sta = ctx.clone_mut().state();
    let vmi = ctx.clone_mut().vm();
    let ctx = ctx.clone_mut();
    let res = vmi.call(ctx, sta, ty, mk, cd, Box::new(pm));
    ctx.depth_set(old_depth);
    let (cost, rv) = res?;
    Ok((cost, Value::bytes(rv)))
}

















/*

fn _setup_vm_run_by_fn(depth: i8, ctx: &mut dyn Context, execfn: impl FnOnce(&mut dyn Context, &mut dyn VM)->Ret<Vec<u8>>) -> Ret<Value> {
    // check tx type
    const TY3: u8 = TransactionType3::TYPE;
    let txty = ctx.env().tx.ty;
    if txty < TY3 {
        return errf!("current transaction type {} too low to setup vm, need at least {}", txty, TY3)
    }
    // init vm
    if false == ctx.vm().active() {
        let vmb = MACHINE_MANAGER.assign(ctx.env().block.height);
        ctx.vm_replace(Box::new(vmb));
    }
    // depth
    let old_depth = ctx.depth();
    ctx.depth_set(depth);
    let ctxptr = ctx as *mut dyn Context;
    let vmptr = ctx.vm() as *mut dyn VM;
    let rv = unsafe {
        let ctx: &mut dyn Context = &mut *ctxptr;
        let vm: &mut dyn VM = &mut *vmptr;
        execfn( ctx, vm )?
    };
    ctx.depth_set(old_depth);
    Ok(Value::bytes(rv))
}

*/