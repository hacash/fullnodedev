pub const AST_TREE_DEPTH_MAX: usize = 6;

pub struct AstNodeTxn {
    snap: CtxSnapshot,
}

impl AstNodeTxn {
    pub fn begin(ctx: &mut dyn Context) -> Ret<Self> {
        let snap = CtxSnapshot::begin(ctx)?;
        Ok(Self { snap })
    }

    pub fn finish<T>(self, ctx: &mut dyn Context, res: Ret<T>) -> Ret<T> {
        let snap = self.snap;
        match res {
            Ok(v) => {
                snap.commit(ctx);
                Ok(v)
            }
            Err(e) => {
                match snap.rollback(ctx) {
                    Ok(()) => Err(e),
                    Err(recover_err) => errf!(
                        "ast node recover failed: {}; original error: {}",
                        recover_err,
                        e
                    ),
                }
            }
        }
    }
}

/// Try executing a child action with an isolated snapshot.
/// On success: charge child's return-gas (size gas) via ctx, then merge.
/// On error: recover snapshot.
macro_rules! ast_try_item {
    ($ctx:expr, $exec:expr) => {{
        let __snap = CtxSnapshot::begin_ast_item($ctx)?;
        let __raw: BRet<(u32, Vec<u8>)> = $exec;
        let __out = match __raw {
            Ok((child_gas, ret)) => {
                $ctx.gas_consume(child_gas)?;
                __snap.commit($ctx);
                Ok(ret)
            }
            Err(e) => {
                if let Err(re) = __snap.rollback($ctx) {
                    return errf!("ast item recover failed: {}; original error: {}", re, e);
                }
                Err(e)
            }
        };
        __out
    }};
}

pub fn validate_ast_select(min: usize, max: usize, num: usize) -> Ret<()> {
    if min > max {
        return errf!("action ast select max cannot less than min");
    }
    if max > num {
        return errf!("action ast select max cannot more than list num");
    }
    if num > TX_ACTIONS_MAX {
        return errf!("action ast select num cannot more than {}", TX_ACTIONS_MAX);
    }
    Ok(())
}

/// Enter an AST branch node: check depth limit and increment level.
/// Returns a guard that restores level on drop.
pub fn ast_enter(ctx: &mut dyn Context) -> Ret<AstLevelGuard<'_>> {
    let old_level = ctx.level();
    let next = match old_level.checked_add(1) {
        Some(v) => v,
        None => return errf!("ast tree depth overflow"),
    };
    if next > AST_TREE_DEPTH_MAX {
        return errf!(
            "ast tree depth {} exceeded max {}",
            next,
            AST_TREE_DEPTH_MAX
        );
    }
    ctx.level_set(next);
    Ok(AstLevelGuard { ctx, old_level })
}

/// `Ok` => continue with value, `Unwind` => skip (continue without value), `Interrupt` => rethrow.
pub fn ast_unwind_continue<T>(out: BRet<T>) -> Ret<Option<T>> {
    match out {
        Ok(v) => Ok(Some(v)),
        Err(BError::Unwind(_)) => Ok(None),
        Err(e) => Err(e.into()), // BError → Error preserves interrupt semantics
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
