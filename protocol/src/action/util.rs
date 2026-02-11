pub const AST_TREE_DEPTH_MAX: usize = 6;

/// Enter an AST branch node: check depth limit and increment level.
/// Returns a guard that restores level on drop.
pub fn ast_enter(ctx: &mut dyn Context) -> Ret<AstLevelGuard<'_>> {
    let old_level = ctx.level();
    let next = old_level
        .checked_add(1)
        .ok_or_else(|| "ast tree depth overflow".to_owned())?;
    if next > AST_TREE_DEPTH_MAX {
        return errf!("ast tree depth {} exceeded max {}", next, AST_TREE_DEPTH_MAX)
    }
    ctx.level_set(next);
    Ok(AstLevelGuard { ctx, old_level })
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
