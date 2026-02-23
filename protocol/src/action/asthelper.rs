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

#[inline]
fn ast_charge_snapshot_overhead(ctx: &dyn Context, gas: &mut i64, before: i64) {
    gas_add(gas, ast_gas_spent_delta(ctx, before));
}

#[inline]
fn ast_charge_child_result(gas: &mut i64, shared: i64, child_gas: i64) {
    // Invariant: child action gas must be non-negative.
    // `action_define!` initializes action gas from serialized size and AST helpers
    // only add non-negative deltas, so a negative `child_gas` indicates a programmer
    // bug in action implementation rather than a runtime business/system error.
    debug_assert!(
        child_gas >= 0,
        "child action returned negative gas: {}",
        child_gas
    );
    let child_gas = child_gas.max(0);
    let extra = child_gas.saturating_sub(shared).max(0);
    gas_add(gas, extra);
}

pub fn ast_run_item_snapshot<F>(
    ctx: &mut dyn Context,
    gas: &mut i64,
    exec: F,
) -> Ret<BRet<(i64, Vec<u8>)>>
where
    F: FnOnce(&mut dyn Context) -> BRet<(i64, Vec<u8>)>,
{
    let snap_before = ctx.gas_remaining();
    let snap = ast_item_snapshot(ctx)?;
    ast_charge_snapshot_overhead(ctx, gas, snap_before);

    let gas_before = ctx.gas_remaining();
    let out = exec(ctx);
    let shared = ast_gas_spent_delta(ctx, gas_before);
    gas_add(gas, shared);

    match &out {
        Ok((child_gas, _)) => {
            ast_charge_child_result(gas, shared, *child_gas);
            ctx_merge(ctx, snap);
        }
        Err(_) => {
            ctx_recover(ctx, snap)?;
        }
    }
    Ok(out)
}

pub fn ast_run_shared_gas<F>(
    ctx: &mut dyn Context,
    gas: &mut i64,
    exec: F,
) -> Ret<BRet<(i64, Vec<u8>)>>
where
    F: FnOnce(&mut dyn Context) -> BRet<(i64, Vec<u8>)>,
{
    let gas_before = ctx.gas_remaining();
    let out = exec(ctx);
    let shared = ast_gas_spent_delta(ctx, gas_before);
    gas_add(gas, shared);
    if let Ok((child_gas, _)) = &out {
        ast_charge_child_result(gas, shared, *child_gas);
    }
    Ok(out)
}

pub fn ast_with_whole_snapshot<T, F>(ctx: &mut dyn Context, body: F) -> Ret<T>
where
    F: FnOnce(&mut dyn Context) -> Ret<T>,
{
    let whole = ctx_snapshot(ctx)?;
    match body(ctx) {
        Ok(v) => {
            ctx_merge(ctx, whole);
            Ok(v)
        }
        Err(e) => {
            ctx_recover(ctx, whole)?;
            Err(e)
        }
    }
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

/// Convert a `BError` back into `Ret` while preserving unwind/interrupt semantics.
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
