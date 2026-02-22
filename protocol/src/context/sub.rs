
pub struct EmptyState {}
impl State for EmptyState {}


/*
* return old and parent state
*/

fn ctx_state_fork_sub(ctx: &mut dyn Context) -> Arc<Box<dyn State>> {
    let nil = Box::new(EmptyState{});
    let old: Arc<Box<dyn State>> = ctx.state_replace(nil).into();
    let sub = old.fork_sub(Arc::downgrade(&old));
    ctx.state_replace(sub); // drop nil
    old
}


fn ctx_state_merge_sub(ctx: &mut dyn Context, old: Arc<Box<dyn State>>) {
    let nil = Box::new(EmptyState{});
    let mut sub = ctx.state_replace(nil);
    sub.detach();
    let mut old = ctx_state_into_box(old);
    old.merge_sub(sub);
    ctx.state_replace(old);
}

fn ctx_state_recover_sub(ctx: &mut dyn Context, old: Arc<Box<dyn State>>) {
    ctx.state().detach();
    let old = ctx_state_into_box(old);
    ctx.state_replace(old); // drop sub state
}

fn ctx_state_into_box(a: Arc<Box<dyn State>>) -> Box<dyn State> {
    assert_eq!(1, Arc::strong_count(&a));
    assert_eq!(0, Arc::weak_count(&a));
    Arc::into_inner(a).unwrap()
}


/*
* Unified context snapshot / merge / recover
* Captures state + VM volatile + logs in one shot.
*/

pub struct CtxSnapshot {
    state:   Arc<Box<dyn State>>,
    vm_snap: Box<dyn Any>,
    vm_is_nil: bool,
    log_len: usize,
    ctx_snap: Box<dyn Any>,
}

pub fn ctx_snapshot(ctx: &mut dyn Context) -> Ret<CtxSnapshot> {
    let vm = ctx.vm();
    let vm_is_nil = vm.is_nil();
    let vm_snap = vm.snapshot_volatile();
    let log_len = ctx.logs().snapshot_len();
    let ctx_snap = ctx.snapshot_volatile();
    let state = ctx_state_fork_sub(ctx);
    Ok(CtxSnapshot { state, vm_snap, vm_is_nil, log_len, ctx_snap })
}

pub fn ast_item_snapshot(ctx: &mut dyn Context) -> Ret<CtxSnapshot> {
    const SNAPSHOT_TRY_GAS: u32 = 40;
    ctx.gas_consume(SNAPSHOT_TRY_GAS)?;
    ctx_snapshot(ctx)
}

pub fn ctx_merge(ctx: &mut dyn Context, snap: CtxSnapshot) {
    ctx_state_merge_sub(ctx, snap.state);
    // vm + logs keep current (successful) values
}

pub fn ctx_recover(ctx: &mut dyn Context, snap: CtxSnapshot) -> Rerr {
    ctx_state_recover_sub(ctx, snap.state);
    // VM recover rolls back branch-local volatile business state while keeping gas/warmup monotonic.
    let cur_is_nil = ctx.vm().is_nil();
    match (snap.vm_is_nil, cur_is_nil) {
        (true, true) => {}
        (true, false) => {
            ctx.vm().restore_but_keep_warmup();
        }
        (false, false) => {
            ctx.vm().restore_volatile(snap.vm_snap);
        }
        (false, true) => {
            return errf!("vm became nil during AST snapshot/recover")
        }
    }
    ctx.logs().truncate(snap.log_len);
    ctx.restore_volatile(snap.ctx_snap);
    Ok(())
}
