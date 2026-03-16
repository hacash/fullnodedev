
pub const ACTION_CTX_LEVEL_TOP: usize = 0;
pub const ACTION_CTX_LEVEL_CALL_BASE: usize = 100;
pub const TX_ACTIONS_MAX: usize = 200;
pub const ACTION_CTX_LEVEL_CALL_MAIN: usize = ACTION_CTX_LEVEL_CALL_BASE;
pub const ACTION_CTX_LEVEL_CALL_CONTRACT: usize = ACTION_CTX_LEVEL_CALL_BASE + 1;
pub const ACTION_CTX_LEVEL_AST_MAX: usize = ACTION_CTX_LEVEL_CALL_BASE - 1;

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum ActExecFrom {
    TxLoop,
    AstWrap,
    ActionCall,
}

// Placement semantics: except AnyInCall, a level means the action may execute
// on that layer or any shallower layer; Guard also cannot enter ActionCall.
// All Top* variants are handled by dedicated branches in check_action_level
// and never fall through to the generic max_ctx_level() path.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum ActLv {
    TopOnly,          // top, single action only
    TopOnlyWithGuard, // top, single non-guard + guards
    TopUnique,        // top, unique by kind
    Top,              // must on top
    Guard,            // env constraint, tx-loop or AST only (ctx <= 99), never ActionCall
    Ast,              // AST and above (ctx <= 99)
    MainCall,         // up to main call (ctx <= 100)
    ContractCall,     // up to contract call (ctx <= 101)
    AnyInCall,        // only from ActionCall, regardless of ctx_level
    Any,              // any context
}


impl ActLv {

    pub fn max_ctx_level(&self) -> Option<usize> {
        use ActLv::*;
        match self {
            // Top* variants are checked by dedicated branches; this arm is defensive only.
            TopOnly | TopOnlyWithGuard | TopUnique | Top => Some(ACTION_CTX_LEVEL_TOP),
            Guard | Ast => Some(ACTION_CTX_LEVEL_AST_MAX),
            MainCall => Some(ACTION_CTX_LEVEL_CALL_MAIN),
            ContractCall => Some(ACTION_CTX_LEVEL_CALL_CONTRACT),
            AnyInCall => None, // checked by execution origin in check_action_level
            Any => None, // truly unlimited
        }
    }
}
