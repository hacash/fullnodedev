pub const TX_ACTIONS_MAX: usize = 200;

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum ExecFrom {
    Top,
    Ast,
    Call,
}

impl ExecFrom {
    pub const fn name(self) -> &'static str {
        match self {
            Self::Top => "TOP",
            Self::Ast => "AST",
            Self::Call => "CALL",
        }
    }
}

impl core::fmt::Display for ExecFrom {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(self.name())
    }
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum TopRule {
    None,
    Only,
    OnlyCanWithGuard,
    Unique,
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum ActScope {
    Top { rule: TopRule },
    Ast,
    Guard,
    Call { only: bool },
}

impl ActScope {
    pub const TOP: Self = Self::Top {
        rule: TopRule::None,
    };
    pub const TOP_ONLY: Self = Self::Top {
        rule: TopRule::Only,
    };
    pub const TOP_ONLY_CAN_WITH_GUARD: Self = Self::Top {
        rule: TopRule::OnlyCanWithGuard,
    };
    pub const TOP_UNIQUE: Self = Self::Top {
        rule: TopRule::Unique,
    };
    pub const AST: Self = Self::Ast;
    pub const GUARD: Self = Self::Guard;
    pub const CALL: Self = Self::Call { only: false };
    pub const CALL_ONLY: Self = Self::Call { only: true };

    pub const fn top_rule(self) -> Option<TopRule> {
        match self {
            Self::Top { rule } => Some(rule),
            _ => None,
        }
    }

    pub const fn name(self) -> &'static str {
        match self {
            Self::Top {
                rule: TopRule::None,
            } => "TOP",
            Self::Top {
                rule: TopRule::Only,
            } => "TOP_ONLY",
            Self::Top {
                rule: TopRule::OnlyCanWithGuard,
            } => "TOP_ONLY_CAN_WITH_GUARD",
            Self::Top {
                rule: TopRule::Unique,
            } => "TOP_UNIQUE",
            Self::Ast => "AST",
            Self::Guard => "GUARD",
            Self::Call { only: false } => "CALL",
            Self::Call { only: true } => "CALL_ONLY",
        }
    }

    pub const fn allows(self, from: ExecFrom) -> bool {
        match self {
            Self::Top { .. } => matches!(from, ExecFrom::Top),
            Self::Ast | Self::Guard => !matches!(from, ExecFrom::Call),
            Self::Call { only: false } => true,
            Self::Call { only: true } => matches!(from, ExecFrom::Call),
        }
    }
}

impl core::fmt::Display for ActScope {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(self.name())
    }
}
