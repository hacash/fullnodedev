
pub const ACTION_CTX_LEVEL_TOP: usize = 0;
pub const ACTION_CTX_LEVEL_CALL_BASE: usize = 100;
pub const TX_ACTIONS_MAX: usize = 200;
pub const ACTION_CTX_LEVEL_CALL_MAIN: usize = ACTION_CTX_LEVEL_CALL_BASE;
pub const ACTION_CTX_LEVEL_CALL_CONTRACT: usize = ACTION_CTX_LEVEL_CALL_BASE + 1;
pub const ACTION_CTX_LEVEL_AST_MAX: usize = ACTION_CTX_LEVEL_CALL_BASE - 1;


#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum ActLv {
    TopOnly,      // only this single one on top
    TopUnique,    // top and unique
    Top,          // must on top
    Ast,          // in act cond AST
    MainCall,     // tx main call (depth=0)
    ContractCall, // contract/abst call
    Any,          // any context
}


impl ActLv {

    pub fn max_ctx_level(&self) -> Option<usize> {
        use ActLv::*;
        match self {
            TopOnly | TopUnique | Top => Some(ACTION_CTX_LEVEL_TOP),
            Ast => Some(ACTION_CTX_LEVEL_AST_MAX),
            MainCall => Some(ACTION_CTX_LEVEL_CALL_MAIN),
            ContractCall => Some(ACTION_CTX_LEVEL_CALL_CONTRACT),
            Any => None, // truly unlimited
        }
    }
}
