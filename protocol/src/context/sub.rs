
pub struct EmptyState {}
impl State for EmptyState {}


/*
* return old and parent state
*/

fn ctx_state_fork_sub(ctx: &mut ContextInst<'_>) -> Arc<Box<dyn State>> {
    let nil = Box::new(EmptyState{});
    let old: Arc<Box<dyn State>> = ctx.state_replace(nil).into();
    let sub = old.fork_sub(Arc::downgrade(&old));
    ctx.state_replace(sub); // drop nil
    old
}


fn ctx_state_merge_sub(ctx: &mut ContextInst<'_>, old: Arc<Box<dyn State>>) {
    let nil = Box::new(EmptyState{});
    let mut sub = ctx.state_replace(nil);
    sub.detach();
    let mut old = ctx_state_into_box(old);
    old.merge_sub(sub);
    ctx.state_replace(old);
}

fn ctx_state_recover_sub(ctx: &mut ContextInst<'_>, old: Arc<Box<dyn State>>) {
    ctx.state().detach();
    let old = ctx_state_into_box(old);
    ctx.state_replace(old); // drop sub state
}

fn ctx_state_into_box(a: Arc<Box<dyn State>>) -> Box<dyn State> {
    assert_eq!(1, Arc::strong_count(&a));
    // Weak references are expected when sub-states keep a parent backlink.
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

impl CtxSnapshot {
    pub fn begin(ctx: &mut dyn Context) -> Ret<Self> {
        let vm = ctx.vm();
        let vm_is_nil = vm.is_nil();
        let vm_snap = vm.snapshot_volatile();
        let log_len = ctx.logs().snapshot_len();
        let ctx_snap = ctx.snapshot_volatile();
        let state = ctx.state_fork();
        Ok(Self { state, vm_snap, vm_is_nil, log_len, ctx_snap })
    }

    pub fn begin_ast_item(ctx: &mut dyn Context) -> Ret<Self> {
        const SNAPSHOT_TRY_GAS: u32 = 40;
        ctx.gas_consume(SNAPSHOT_TRY_GAS)?;
        Self::begin(ctx)
    }

    pub fn commit(self, ctx: &mut dyn Context) {
        ctx.state_merge(self.state);
        // vm + logs keep current (successful) values
    }

    pub fn rollback(self, ctx: &mut dyn Context) -> Rerr {
        ctx.state_recover(self.state);
        // VM recover rolls back branch-local volatile business state while keeping gas/warmup monotonic.
        let cur_is_nil = ctx.vm().is_nil();
        match (self.vm_is_nil, cur_is_nil) {
            (true, true) => {}
            (true, false) => {
                ctx.vm().restore_but_keep_warmup();
            }
            (false, false) => {
                ctx.vm().restore_volatile(self.vm_snap);
            }
            (false, true) => {
                return errf!("vm became nil during AST snapshot/recover")
            }
        }
        ctx.logs().truncate(self.log_len);
        ctx.restore_volatile(self.ctx_snap);
        Ok(())
    }
}
