
pub struct EmptyState {}
impl State for EmptyState {}

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
    vm_snap: Option<Box<dyn Any>>,
    log_len: usize,
    ctx_snap: Box<dyn Any>,
}

impl CtxSnapshot {
    pub fn begin(ctx: &mut dyn Context) -> Ret<Self> {
        let vm_snap = ctx.vm_snapshot_volatile();
        let log_len = ctx.logs().snapshot_len();
        let ctx_snap = ctx.snapshot_volatile();
        let state = ctx.state_fork();
        Ok(Self { state, vm_snap, log_len, ctx_snap })
    }

    pub fn begin_ast_item(ctx: &mut dyn Context) -> Ret<Self> {
        const SNAPSHOT_TRY_GAS: u32 = 40;
        ctx.gas_charge(SNAPSHOT_TRY_GAS as i64)?;
        Self::begin(ctx)
    }

    pub fn commit(self, ctx: &mut dyn Context) {
        ctx.state_merge(self.state);
        // vm + logs keep current (successful) values
    }

    pub fn rollback(self, ctx: &mut dyn Context) -> Rerr {
        ctx.state_recover(self.state);
        let cur_snap = ctx.vm_snapshot_volatile();
        match (self.vm_snap, cur_snap) {
            (None, None) => {}
            (None, Some(_)) => {
                ctx.vm_restore_but_keep_warmup();
            }
            (Some(snap), Some(_)) => {
                ctx.vm_restore_volatile(snap);
            }
            (Some(_), None) => {
                return errf!("vm disappeared during AST snapshot/recover")
            }
        }
        ctx.logs().truncate(self.log_len);
        ctx.restore_volatile(self.ctx_snap);
        Ok(())
    }
}
