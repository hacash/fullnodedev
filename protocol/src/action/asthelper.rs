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
            Err(e) => match snap.rollback(ctx) {
                Ok(()) => Err(e),
                Err(recover_err) => errf!(
                    "ast node recover failed: {}; original error: {}",
                    recover_err,
                    e
                ),
            },
        }
    }
}

/// Try executing a child action with an isolated snapshot.
/// On success: charge child's return-gas (size gas) via ctx, then merge.
/// On error: recover snapshot.
macro_rules! ast_try_item {
    ($ctx:expr, $exec:expr, $child_burn90:expr) => {{
        let __child_burn90 = $child_burn90;
        let __snap = CtxSnapshot::begin_ast_item($ctx)?;
        let __raw: XRet<(u32, Vec<u8>)> = $exec;
        let __out = match __raw {
            Ok((child_gas, ret)) => {
                let charge_gas = crate::context::apply_burn90_multiplier(
                    $ctx.tx().burn_90(),
                    __child_burn90,
                    child_gas,
                );
                if let Err(gas_err) = $ctx.gas_charge(charge_gas as i64) {
                    if let Err(re) = __snap.rollback($ctx) {
                        return errf!(
                            "ast item recover failed: {}; original error: {}",
                            re,
                            gas_err
                        );
                    }
                    Err(gas_err.into())
                } else {
                    __snap.commit($ctx);
                    Ok(ret)
                }
            }
            Err(e) => {
                if let Err(re) = __snap.rollback($ctx) {
                    return errf!("ast item recovery failed: {}; original error: {}", re, e);
                }
                Err(e)
            }
        };
        __out
    }};
    ($ctx:expr, $exec:expr) => {{
        ast_try_item!($ctx, $exec, false)
    }};
}

pub fn validate_ast_select(min: usize, max: usize, num: usize) -> Ret<()> {
    if min > max {
        return errf!("action ast select max cannot be less than min");
    }
    if max > num {
        return errf!("action ast select max cannot exceed list num");
    }
    if num > TX_ACTIONS_MAX {
        return errf!("action ast select num cannot exceed {}", TX_ACTIONS_MAX);
    }
    Ok(())
}

pub(crate) fn is_ast_container_action(act: &dyn Action) -> bool {
    act.as_any().downcast_ref::<AstSelect>().is_some() || act.as_any().downcast_ref::<AstIf>().is_some()
}

pub(crate) fn get_action_childs<'a>(act: &'a dyn Action) -> Option<Vec<&'a dyn Action>> {
    if let Some(ast) = act.as_any().downcast_ref::<AstSelect>() {
        return Some(ast.actions.as_list().iter().map(|sub| sub.as_ref()).collect());
    }
    if let Some(ast) = act.as_any().downcast_ref::<AstIf>() {
        let cond = ast.cond.actions.as_list();
        let br_if = ast.br_if.actions.as_list();
        let br_else = ast.br_else.actions.as_list();
        let mut childs = Vec::with_capacity(cond.len() + br_if.len() + br_else.len());
        childs.extend(cond.iter().map(|sub| sub.as_ref()));
        childs.extend(br_if.iter().map(|sub| sub.as_ref()));
        childs.extend(br_else.iter().map(|sub| sub.as_ref()));
        return Some(childs);
    }
    None
}

/// Enter an AST branch node: increment level and set exec_from to AstWrap.
/// Returns a guard that restores both level and exec_from on drop.
pub fn ast_enter(ctx: &mut dyn Context) -> Ret<AstLevelGuard<'_>> {
    let old_level = ctx.level();
    let old_exec_from = ctx.action_exec_from();
    let next = match old_level.checked_add(1) {
        Some(v) => v,
        None => return errf!("ast ctx level overflow"),
    };
    ctx.level_set(next);
    ctx.action_exec_from_set(ActExecFrom::AstWrap);
    Ok(AstLevelGuard {
        ctx,
        old_level,
        old_exec_from,
    })
}

/// `Ok` => continue with value, `Revert` => skip (continue without value), `Fault` => rethrow.
pub fn ast_revert_continue<T>(out: XRet<T>) -> Ret<Option<T>> {
    match out {
        Ok(v) => Ok(Some(v)),
        Err(XError::Revert(_)) => Ok(None),
        Err(e) => Err(e.into()), // XError → Error preserves fault semantics
    }
}

pub struct AstLevelGuard<'a> {
    ctx: &'a mut dyn Context,
    old_level: usize,
    old_exec_from: ActExecFrom,
}

impl AstLevelGuard<'_> {
    pub fn ctx(&mut self) -> &mut dyn Context {
        self.ctx
    }
}

impl Drop for AstLevelGuard<'_> {
    fn drop(&mut self) {
        self.ctx.level_set(self.old_level);
        self.ctx.action_exec_from_set(self.old_exec_from);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::ContextInst;
    use crate::state::EmptyLogs;
    use crate::transaction::TransactionType2;

    #[derive(Default, Clone)]
    struct TestForkState {
        parent: std::sync::Weak<Box<dyn State>>,
        mem: MemMap,
    }

    impl State for TestForkState {
        fn fork_sub(&self, parent: std::sync::Weak<Box<dyn State>>) -> Box<dyn State> {
            Box::new(Self {
                parent,
                mem: MemMap::default(),
            })
        }

        fn merge_sub(&mut self, sta: Box<dyn State>) {
            self.mem.extend(sta.as_mem().clone());
        }

        fn detach(&mut self) {
            self.parent = std::sync::Weak::<Box<dyn State>>::new();
        }

        fn clone_state(&self) -> Box<dyn State> {
            Box::new(self.clone())
        }

        fn as_mem(&self) -> &MemMap {
            &self.mem
        }

        fn get(&self, k: Vec<u8>) -> Option<Vec<u8>> {
            if let Some(v) = self.mem.get(&k) {
                return v.clone();
            }
            if let Some(parent) = self.parent.upgrade() {
                return parent.get(k);
            }
            None
        }

        fn set(&mut self, k: Vec<u8>, v: Vec<u8>) {
            self.mem.insert(k, Some(v));
        }

        fn del(&mut self, k: Vec<u8>) {
            self.mem.insert(k, None);
        }
    }

    fn run_try_item_gas_fail(ctx: &mut dyn Context) -> Ret<XRet<Vec<u8>>> {
        Ok(ast_try_item!(ctx, Ok((1u32, vec![1u8]))))
    }

    #[test]
    fn test_ast_try_item_gas_fail_must_rollback_item_snapshot() {
        let tx = TransactionType2::new_by(
            field::ADDRESS_ONEX.clone(),
            Amount::unit238(1000),
            1730000000,
        );
        let mut env = Env::default();
        env.chain.fast_sync = true;
        env.tx = crate::transaction::create_tx_info(&tx);
        let mut ctx = ContextInst::new(
            env,
            Box::new(TestForkState::default()),
            Box::new(EmptyLogs {}),
            &tx,
        );
        {
            let main = field::ADDRESS_ONEX.clone();
            let mut state = crate::state::CoreState::wrap(ctx.state());
            let mut bls = state.balance(&main).unwrap_or_default();
            bls.hacash = Amount::unit238(1_000_000_000);
            state.balance_set(&main, &bls);
        }
        let key = b"parent-key".to_vec();
        let val = b"parent-val".to_vec();
        ctx.state().set(key.clone(), val.clone());
        ctx.gas_init_tx(40, 1).unwrap();
        let out = run_try_item_gas_fail(&mut ctx).unwrap();
        let err = out.unwrap_err();
        assert!(err.is_fault(), "{}", err);
        assert!(err.contains("gas has run out"), "{}", err);
        assert_eq!(ctx.state().get(key), Some(val));
    }
}
