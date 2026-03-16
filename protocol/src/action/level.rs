pub fn check_action_ast_tree_depth(act: &dyn Action) -> Rerr {
    fn check_depth(act: &dyn Action, ast_depth: usize) -> Rerr {
        let Some((inc, childs)) = get_action_level_inc_and_childs(act) else {
            return Ok(());
        };
        let next_depth = match ast_depth.checked_add(inc) {
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
}

impl TxActionAnalyzeStats {
    fn record_top(&mut self, act: &dyn Action) {
        let kid = act.kind();
        *self.top_kind_count.entry(kid).or_insert(0) += 1;
    }

    fn record_leaf(&mut self, scope: ActScope) {
        if scope == ActScope::GUARD {
            self.has_guard_leaf = true;
        } else {
            self.non_guard_leaf_count += 1;
        }
    }

    fn check_not_all_guard(&self) -> Rerr {
        if self.has_guard_leaf && self.non_guard_leaf_count == 0 {
            return errf!("tx actions cannot be all GUARD");
        }
        Ok(())
    }

    fn check_top_rule(&self, act: &dyn Action, actlen: usize) -> Rerr {
        let kid = act.kind();
        let Some(rule) = act.scope().top_rule() else {
            return Ok(());
        };
        match rule {
            TopRule::None => Ok(()),
            TopRule::Only if actlen != 1 => {
                errf!("action {} can only execute on TOP_ONLY", kid)
            }
            TopRule::OnlyWithGuard if self.non_guard_leaf_count != 1 => {
                errf!(
                    "action {} can only execute on TOP_ONLY_WITH_GUARD (requires exactly one non-guard leaf action)",
                    kid
                )
            }
            TopRule::Unique if self.top_kind_count.get(&kid).copied().unwrap_or(0) != 1 => {
                errf!("action {} can only execute on TOP_UNIQUE", kid)
            }
            _ => Ok(()),
        }
    }
}

fn scan_action_node(act: &dyn Action, stats: &mut TxActionAnalyzeStats, is_top: bool) -> Rerr {
    let kid = act.kind();
    let scope = act.scope();
    let exec_from = maybe!(is_top, ExecFrom::Top, ExecFrom::Ast);
    if !scope.allows(exec_from) {
        return maybe!(
            is_top,
            errf!(
                "action {} with scope {} cannot appear at tx top level",
                kid,
                scope
            ),
            errf!(
                "action {} with scope {} cannot appear inside AST",
                kid,
                scope
            )
        );
    }
    if is_top {
        stats.record_top(act);
    }
    if let Some((_, childs)) = get_action_level_inc_and_childs(act) {
        for sub in childs {
            scan_action_node(sub, stats, false)?;
        }
    } else {
        stats.record_leaf(scope);
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
    let mut stats = TxActionAnalyzeStats::default();
    for act in actions {
        scan_action_node(act.as_ref(), &mut stats, true)?;
    }
    stats.check_not_all_guard()?;
    for act in actions {
        stats.check_top_rule(act.as_ref(), actlen)?;
    }
    Ok(())
}

pub fn check_action_scope(exec_from: ExecFrom, act: &dyn Action) -> Rerr {
    let scope = act.scope();
    if !scope.allows(exec_from) {
        return errf!(
            "action {} with scope {} not allowed from {}",
            act.kind(),
            scope,
            exec_from
        );
    }
    Ok(())
}
