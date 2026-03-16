
pub fn check_action_ast_tree_depth(act: &dyn Action) -> Rerr {
    fn check_depth(act: &dyn Action, ast_depth: usize) -> Rerr {
        let Some(childs) = get_action_childs(act) else {
            return Ok(());
        };
        let next_depth = match ast_depth.checked_add(1) {
            Some(v) => v,
            None => return errf!("ast tree depth overflow"),
        };
        if next_depth > AST_TREE_DEPTH_MAX {
            return errf!(
                "ast tree depth {} exceeded max {}",
                next_depth,
                AST_TREE_DEPTH_MAX
            );
        }
        for sub in childs {
            check_depth(sub, next_depth)?;
        }
        Ok(())
    }

    check_depth(act, 0)
}

pub fn check_tx_action_ast_tree_depth(actions: &Vec<Box<dyn Action>>) -> Rerr {
    for act in actions {
        check_action_ast_tree_depth(act.as_ref())?;
    }
    Ok(())
}

#[derive(Default)]
struct TxActionAnalyzeStats {
    non_guard_leaf_count: usize,
    top_kind_count: std::collections::HashMap<u16, usize>,
    has_guard_leaf: bool,
    has_non_guard_leaf: bool,
}

impl TxActionAnalyzeStats {
    fn record_top(&mut self, act: &dyn Action) {
        let kid = act.kind();
        *self.top_kind_count.entry(kid).or_insert(0) += 1;
    }

    fn record_leaf(&mut self, alv: ActLv) {
        if alv == ActLv::Guard {
            self.has_guard_leaf = true;
        } else {
            self.has_non_guard_leaf = true;
            self.non_guard_leaf_count += 1;
        }
    }

    fn check_not_all_guard(&self) -> Rerr {
        if self.has_guard_leaf && !self.has_non_guard_leaf {
            return errf!("tx actions cannot be all GUARD");
        }
        Ok(())
    }

    fn check_top_rule(&self, act: &dyn Action, actlen: usize) -> Rerr {
        let kid = act.kind();
        match act.level() {
            ActLv::TopOnly if actlen != 1 => {
                errf!("action {} can only execute on TOP_ONLY", kid)
            }
            ActLv::TopOnlyWithGuard if self.non_guard_leaf_count != 1 => {
                errf!(
                    "action {} can only execute on TOP_ONLY_WITH_GUARD (requires exactly one non-guard leaf action)",
                    kid
                )
            }
            ActLv::TopUnique if self.top_kind_count.get(&kid).copied().unwrap_or(0) != 1 => {
                errf!("action {} can only execute on level TOP_UNIQUE", kid)
            }
            _ => Ok(()),
        }
    }
}

fn scan_nested_ast_action(act: &dyn Action, stats: &mut TxActionAnalyzeStats) -> Rerr {
    let kid = act.kind();
    let alv = act.level();
    match alv {
        ActLv::TopOnly => return errf!("action {} can only execute on TOP_ONLY", kid),
        ActLv::TopOnlyWithGuard => {
            return errf!("action {} can only execute on TOP_ONLY_WITH_GUARD", kid)
        }
        ActLv::TopUnique => {
            return errf!("action {} can only execute on TOP_UNIQUE", kid)
        }
        ActLv::Top => return errf!("action {} can only execute on TOP", kid),
        ActLv::AnyInCall => {
            return errf!("action {} can only execute from ActionCall context", kid)
        }
        _ => {}
    }
    if let Some(childs) = get_action_childs(act) {
        for sub in childs {
            scan_nested_ast_action(sub, stats)?;
        }
    } else {
        stats.record_leaf(alv);
    }
    Ok(())
}

fn scan_top_action(act: &dyn Action, stats: &mut TxActionAnalyzeStats) -> Rerr {
    let kid = act.kind();
    let alv = act.level();
    stats.record_top(act);
    match alv {
        ActLv::AnyInCall => {
            return errf!("action {} can only execute from ActionCall context", kid)
        }
        _ => {}
    }
    if let Some(childs) = get_action_childs(act) {
        for sub in childs {
            scan_nested_ast_action(sub, stats)?;
        }
    } else {
        stats.record_leaf(alv);
    }
    Ok(())
}

pub fn analyze_tx_action_set(actions: &Vec<Box<dyn Action>>) -> Rerr {
    let actlen = actions.len();
    if actlen < 1 || actlen > TX_ACTIONS_MAX {
        return errf!(
            "action length {} is 0 or one transaction max actions is {}",
            actlen,
            TX_ACTIONS_MAX
        );
    }
    check_tx_action_ast_tree_depth(actions)?;
    let mut stats = TxActionAnalyzeStats::default();
    for act in actions {
        scan_top_action(act.as_ref(), &mut stats)?;
    }
    // Guard actions are environment constraints and cannot form a standalone tx,
    // even when wrapped by AST container nodes.
    stats.check_not_all_guard()?;
    for act in actions {
        stats.check_top_rule(act.as_ref(), actlen)?;
    }
    Ok(())
}

// check action level
pub fn check_action_level(ctx_level: usize, exec_from: ActExecFrom, act: &dyn Action) -> Rerr {
    let kid = act.kind();
    macro_rules! check_top_level {
        ($name:literal) => {{
            if exec_from != ActExecFrom::TxLoop {
                return errf!("action {} can only execute in tx action loop", kid);
            }
            if ctx_level != ACTION_CTX_LEVEL_TOP {
                return errf!("action {} can only execute on {}", kid, $name);
            }
        }};
    }
    macro_rules! check_ctx_level {
        (> $max:expr) => {{
            let max = $max;
            if ctx_level > max {
                return errf!("action {} max ctx level {} but call in {}", kid, max, ctx_level);
            }
        }};
        (> $max:expr, $($arg:tt)+) => {{
            let max = $max;
            if ctx_level > max {
                return errf!($($arg)+);
            }
        }};
    }
    match act.level() {
        ActLv::TopOnly => check_top_level!("TOP_ONLY"),
        ActLv::TopOnlyWithGuard => check_top_level!("TOP_ONLY_WITH_GUARD"),
        ActLv::TopUnique => check_top_level!("TOP_UNIQUE"),
        ActLv::Top => check_top_level!("TOP"),
        ActLv::Ast => {
            if exec_from == ActExecFrom::ActionCall {
                return errf!("action {} (AST) cannot execute from ActionCall context", kid);
            }
            check_ctx_level!(> ACTION_CTX_LEVEL_AST_MAX)
        }
        ActLv::MainCall => check_ctx_level!(> ACTION_CTX_LEVEL_CALL_MAIN),
        ActLv::ContractCall => check_ctx_level!(> ACTION_CTX_LEVEL_CALL_CONTRACT),
        ActLv::AnyInCall => {
            if exec_from != ActExecFrom::ActionCall {
                return errf!("action {} can only execute from ActionCall context", kid);
            }
        }
        ActLv::Guard => {
            if exec_from == ActExecFrom::ActionCall {
                return errf!(
                    "action {} (Guard) cannot execute from ActionCall context",
                    kid
                );
            }
            check_ctx_level!(
                > ACTION_CTX_LEVEL_AST_MAX,
                "action {} can only execute on GUARD (TOP + AST, ctx <= {}), now ctx {}",
                kid,
                ACTION_CTX_LEVEL_AST_MAX,
                ctx_level
            );
        }
        ActLv::Any => {}
    }
    // ok
    Ok(())
}
