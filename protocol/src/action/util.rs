pub const AST_TREE_DEPTH_MAX: usize = 6;

pub fn ast_gas_spent_delta(ctx: &dyn Context, before: i64) -> i64 {
    let after = ctx.gas_remaining();
    let spent = before.saturating_sub(after);
    if spent <= 0 {
        0
    } else {
        spent
    }
}

/// Enter an AST branch node: check depth limit and increment level.
/// Returns a guard that restores level on drop.
pub fn ast_enter(ctx: &mut dyn Context) -> Ret<AstLevelGuard<'_>> {
    let old_level = ctx.level();
    let next = match old_level.checked_add(1) {
        Some(v) => v,
        None => return erruf!("ast tree depth overflow"),
    };
    if next > AST_TREE_DEPTH_MAX {
        return erruf!(
            "ast tree depth {} exceeded max {}",
            next,
            AST_TREE_DEPTH_MAX
        );
    }
    ctx.level_set(next);
    Ok(AstLevelGuard { ctx, old_level })
}

/// Convert a `BError` back into `Ret` while preserving recoverable/unrecoverable semantics.
pub fn ast_rethrow<T>(e: BError) -> Ret<T> {
    match e {
        BError::Unwind(msg) => erru!(msg),
        BError::Interrupt(msg) => err!(msg),
    }
}

pub struct AstLevelGuard<'a> {
    ctx: &'a mut dyn Context,
    old_level: usize,
}

impl AstLevelGuard<'_> {
    pub fn ctx(&mut self) -> &mut dyn Context {
        self.ctx
    }
}

impl Drop for AstLevelGuard<'_> {
    fn drop(&mut self) {
        self.ctx.level_set(self.old_level);
    }
}
