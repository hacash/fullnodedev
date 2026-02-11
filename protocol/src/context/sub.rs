
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
    log_len: usize,
    ctx_snap: Box<dyn Any>,
}

pub fn ctx_snapshot(ctx: &mut dyn Context) -> CtxSnapshot {
    let vm_snap = ctx.vm().snapshot_volatile();
    let log_len = ctx.logs().snapshot_len();
    let ctx_snap = ctx.snapshot_volatile();
    let state = ctx_state_fork_sub(ctx);
    CtxSnapshot { state, vm_snap, log_len, ctx_snap }
}

pub fn ctx_merge(ctx: &mut dyn Context, snap: CtxSnapshot) {
    ctx_state_merge_sub(ctx, snap.state);
    // vm + logs keep current (successful) values
}

pub fn ctx_recover(ctx: &mut dyn Context, snap: CtxSnapshot) {
    ctx_state_recover_sub(ctx, snap.state);
    // VM recover intentionally excludes gas remaining:
    // failed AST branches rollback state/log/memory, but consumed gas is not refunded.
    ctx.vm().restore_volatile(snap.vm_snap);
    ctx.logs().truncate(snap.log_len);
    ctx.restore_volatile(snap.ctx_snap);
}
