struct AstChildren<'a> {
    depth_inc: usize,
    childs: Vec<&'a dyn Action>,
}

#[derive(Clone, Copy)]
enum LevelCheckMode {
    TxPrecheck { fast_sync: bool },
    Runtime,
}

#[derive(Default)]
struct LevelCheckState {
    top_action_count: usize,
    top_kind_count: std::collections::HashMap<u16, usize>,
    top_guard_count: usize,
    non_guard_leaf_count: usize,
    has_guard_leaf: bool,
}

impl LevelCheckState {
    fn record_top_action(&mut self, act: &dyn Action) {
        self.top_action_count += 1;
        *self.top_kind_count.entry(act.kind()).or_insert(0) += 1;
        if act.scope() == ActScope::GUARD {
            self.top_guard_count += 1;
        }
    }

    fn record_leaf_action(&mut self, scope: ActScope) {
        if scope == ActScope::GUARD {
            self.has_guard_leaf = true;
        } else {
            self.non_guard_leaf_count += 1;
        }
    }
}

fn action_ast_children<'a>(act: &'a dyn Action) -> Option<AstChildren<'a>> {
    let (depth_inc, childs) = get_action_level_inc_and_childs(act)?;
    Some(AstChildren { depth_inc, childs })
}

fn check_depth_limit(ast_depth: usize, depth_inc: usize) -> Ret<usize> {
    let next_depth = ast_depth
        .checked_add(depth_inc)
        .ok_or_else(|| "ast tree depth overflow".to_string())?;
    if next_depth > AST_TREE_DEPTH_MAX {
        return errf!(
            "ast tree depth {} exceeded max {}",
            next_depth,
            AST_TREE_DEPTH_MAX
        );
    }
    Ok(next_depth)
}

fn check_node_tx_type(tx_type: u8, act: &dyn Action) -> Rerr {
    let min_tx_type = act.min_tx_type();
    if tx_type < min_tx_type {
        return errf!(
            "action {} requires tx type >= {} but current tx type is {}",
            act.kind(),
            min_tx_type,
            tx_type
        );
    }
    Ok(())
}

fn check_node_scope(origin: ExecFrom, act: &dyn Action) -> Rerr {
    let scope = act.scope();
    if !scope.allows(origin) {
        return errf!(
            "action {} with scope {} not allowed from {}",
            act.kind(),
            scope,
            origin
        );
    }
    Ok(())
}

fn check_no_guard_only_tx(state: &LevelCheckState) -> Rerr {
    if state.has_guard_leaf && state.non_guard_leaf_count == 0 {
        return errf!("tx actions cannot be all GUARD");
    }
    Ok(())
}

fn check_top_only_rule(act: &dyn Action, state: &LevelCheckState) -> Rerr {
    if state.top_action_count != 1 {
        return errf!("action {} can only execute on TOP_ONLY", act.kind());
    }
    Ok(())
}

fn check_top_only_can_with_guard_rule(act: &dyn Action, state: &LevelCheckState) -> Rerr {
    if state.top_action_count != 1 + state.top_guard_count || state.non_guard_leaf_count != 1 {
        return errf!(
            "action {} can only execute on TOP_ONLY_CAN_WITH_GUARD (requires exactly one non-guard leaf action plus optional bare top-level GUARD actions)",
            act.kind()
        );
    }
    Ok(())
}

fn check_top_unique_rule(act: &dyn Action, state: &LevelCheckState) -> Rerr {
    if state.top_kind_count.get(&act.kind()).copied().unwrap_or(0) != 1 {
        return errf!("action {} can only execute on TOP_UNIQUE", act.kind());
    }
    Ok(())
}

fn check_top_rule(act: &dyn Action, state: &LevelCheckState) -> Rerr {
    let Some(rule) = act.scope().top_rule() else {
        return Ok(());
    };
    match rule {
        TopRule::None => Ok(()),
        TopRule::Only => check_top_only_rule(act, state),
        TopRule::OnlyCanWithGuard => check_top_only_can_with_guard_rule(act, state),
        TopRule::Unique => check_top_unique_rule(act, state),
    }
}

fn visit_node(
    tx_type: u8,
    act: &dyn Action,
    origin: ExecFrom,
    ast_depth: usize,
    mode: LevelCheckMode,
    state: &mut LevelCheckState,
) -> Rerr {
    let do_scope_check = match mode {
        LevelCheckMode::TxPrecheck { fast_sync } => !fast_sync,
        LevelCheckMode::Runtime => true,
    };
    if do_scope_check {
        check_node_scope(origin, act)?;
    }

    if matches!(mode, LevelCheckMode::TxPrecheck { fast_sync: false }) && matches!(origin, ExecFrom::Top) {
        state.record_top_action(act);
    }

    let Some(children) = action_ast_children(act) else {
        check_node_tx_type(tx_type, act)?;
        if matches!(mode, LevelCheckMode::TxPrecheck { fast_sync: false }) {
            state.record_leaf_action(act.scope());
        }
        return Ok(());
    };

    let next_depth = check_depth_limit(ast_depth, children.depth_inc)?;
    let child_origin = ExecFrom::Ast;
    for sub in children.childs {
        visit_node(tx_type, sub, child_origin, next_depth, mode, state)?;
    }
    check_node_tx_type(tx_type, act)?;
    Ok(())
}

pub fn precheck_tx_actions(tx_type: u8, fast_sync: bool, actions: &Vec<Box<dyn Action>>) -> Rerr {
    let actlen = actions.len();
    if actlen < 1 || actlen > TX_ACTIONS_MAX {
        return errf!(
            "action length {} is 0 or one transaction max actions is {}",
            actlen,
            TX_ACTIONS_MAX
        );
    }
    let mut state = LevelCheckState::default();
    for act in actions {
        visit_node(
            tx_type,
            act.as_ref(),
            ExecFrom::Top,
            0,
            LevelCheckMode::TxPrecheck { fast_sync },
            &mut state,
        )?;
    }
    check_no_guard_only_tx(&state)?;
    for act in actions {
        check_top_rule(act.as_ref(), &state)?;
    }
    Ok(())
}

pub fn precheck_runtime_action(tx_type: u8, act: &dyn Action, from: ExecFrom) -> Rerr {
    let mut state = LevelCheckState::default();
    visit_node(tx_type, act, from, 0, LevelCheckMode::Runtime, &mut state)?;
    Ok(())
}
