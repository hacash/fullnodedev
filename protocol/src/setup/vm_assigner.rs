

/*
    VM assigner: allows vm crate to register its assign function
    so that protocol layer can pre-initialize VM at TX execution entry.
*/

pub type FnVmAssignFunc = fn(height: u64) -> Box<dyn VM>;

pub static mut VM_ASSIGN_FUNC: Option<FnVmAssignFunc> = None;

pub fn vm_assigner(f: FnVmAssignFunc) {
    unsafe {
        VM_ASSIGN_FUNC = Some(f);
    }
}

fn tx_vm_enabled(ctx: &mut dyn Context) -> bool {
    if ctx.env().tx.ty < crate::transaction::TransactionType3::TYPE {
        return false
    }
    matches!(ctx.tx().fee_extend(), Ok(v) if v > 0)
}

/// Initialize VM on context if an assigner is registered and VM is not yet created.
pub fn do_vm_init(ctx: &mut dyn Context) {
    if ctx.vm().usable() || !tx_vm_enabled(ctx) {
        return
    }
    let assign = unsafe { VM_ASSIGN_FUNC };
    if let Some(f) = assign {
        let vm = f(ctx.env().block.height);
        ctx.vm_replace(vm);
    }
}
