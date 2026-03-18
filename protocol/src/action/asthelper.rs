pub const AST_TREE_DEPTH_MAX: usize = 6;

#[inline]
fn ast_charge_returned_gas(ctx: &mut dyn Context, child_extra9: bool, child_gas: u32) -> XRet<()> {
    // AST child returned-gas follows the same delta-only extra9 rule as other returned-gas charge sites.
    let charge_gas = crate::context::apply_extra9_surcharge(child_extra9, child_gas);
    ctx.gas_charge(charge_gas as i64).into_xret()
}

/// Execute one AST child inside an isolated snapshot.
/// - success: charge returned-gas and commit the child snapshot
/// - recoverable failure: rollback only this child snapshot
/// - unrecoverable failure: return immediately and let upper layers own rollback
pub fn ast_exec_item<T>(
    ctx: &mut dyn Context,
    child_extra9: bool,
    exec: impl FnOnce(&mut dyn Context) -> XRet<(u32, T)>,
) -> XRet<T> {
    let snap = CtxSnapshot::begin_ast_item(ctx).into_xret()?;
    match exec(ctx) {
        Ok((child_gas, ret)) => {
            ast_charge_returned_gas(ctx, child_extra9, child_gas)?;
            snap.commit(ctx);
            Ok(ret)
        }
        Err(XError::Revert(msg)) => {
            snap.rollback(ctx).into_xret()?;
            Err(XError::revert(msg))
        }
        Err(e) => Err(e),
    }
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
    act.as_any().downcast_ref::<AstSelect>().is_some()
        || act.as_any().downcast_ref::<AstIf>().is_some()
}

pub(crate) fn get_action_level_inc_and_childs<'a>(
    act: &'a dyn Action,
) -> Option<(usize, Vec<&'a dyn Action>)> {
    if let Some(ast) = act.as_any().downcast_ref::<AstSelect>() {
        return Some((
            1,
            ast.actions
                .as_list()
                .iter()
                .map(|sub| sub.as_ref())
                .collect(),
        ));
    }
    if let Some(ast) = act.as_any().downcast_ref::<AstIf>() {
        let cond = ast.cond.actions.as_list();
        let br_if = ast.br_if.actions.as_list();
        let br_else = ast.br_else.actions.as_list();
        let mut childs = Vec::with_capacity(cond.len() + br_if.len() + br_else.len());
        childs.extend(cond.iter().map(|sub| sub.as_ref()));
        childs.extend(br_if.iter().map(|sub| sub.as_ref()));
        childs.extend(br_else.iter().map(|sub| sub.as_ref()));
        // Weighted AST depth keeps AstIf aligned with current control-flow charging:
        // one level for evaluating `cond`, one level for executing the selected branch.
        return Some((2, childs));
    }
    None
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
        Ok(ast_exec_item(ctx, true, |_ctx| Ok((5u32, vec![1u8]))))
    }

    fn run_try_item_revert_after_write(
        ctx: &mut dyn Context,
        key: u8,
        val: u8,
    ) -> Ret<XRet<Vec<u8>>> {
        Ok(ast_exec_item(ctx, false, |ctx| {
            ctx.state().set(vec![key], vec![val]);
            Err(XError::revert("ast test recoverable fail"))
        }))
    }

    fn run_try_item_fault_after_write(
        ctx: &mut dyn Context,
        key: u8,
        val: u8,
    ) -> Ret<XRet<Vec<u8>>> {
        Ok(ast_exec_item(ctx, false, |ctx| {
            ctx.state().set(vec![key], vec![val]);
            Err(XError::fault("ast test unrecoverable fail"))
        }))
    }

    #[test]
    fn test_ast_try_item_revert_recovers_only_child_snapshot() {
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
        ctx.gas_init_tx(1000, 1).unwrap();
        ctx.state().set(vec![1], vec![1]);

        let err = run_try_item_revert_after_write(&mut ctx, 2, 2)
            .unwrap()
            .unwrap_err();
        assert!(err.is_revert(), "{err}");
        assert_eq!(ctx.state().get(vec![1]), Some(vec![1]));
        assert_eq!(ctx.state().get(vec![2]), None);
    }

    #[test]
    fn test_ast_try_item_fault_fast_fails_without_child_recover() {
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
        ctx.gas_init_tx(1000, 1).unwrap();
        ctx.state().set(vec![1], vec![1]);

        let err = run_try_item_fault_after_write(&mut ctx, 2, 2)
            .unwrap()
            .unwrap_err();
        assert!(err.is_fault(), "{err}");
    }
}
