#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CallMode {
    External,
    Inner,
    View,
    Pure,
}

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum EntryKind {
    #[default]
    Main,
    P2sh,
    Abst,
}

enum_try_from_u8_by_variant!(
    EntryKind,
    ItrErrCode::CallInvalid,
    "entry kind {} not find",
    [Main, P2sh, Abst]
);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum EffectMode {
    #[default]
    Edit,
    View,
    Pure,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct FrameBindings {
    pub context_addr: ContractAddress,
    pub state_addr: Option<ContractAddress>,
    pub code_owner: Option<ContractAddress>,
    pub lib_table: Arc<[ContractAddress]>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ExecCtx {
    pub entry: EntryKind,
    pub effect: EffectMode,
    pub call_depth: usize,
}

#[derive(Debug, Clone)]
pub enum CallExit {
    Abort,
    Throw,
    Finish,
    Return,
    Call(UserCall),
}

impl EntryKind {
    pub const fn root_exec(self) -> ExecCtx {
        match self {
            Self::Main => ExecCtx::main(),
            Self::P2sh => ExecCtx::p2sh(),
            Self::Abst => ExecCtx::abst(),
        }
    }
}

impl CallMode {
    pub const fn to_effect(self) -> EffectMode {
        match self {
            Self::External | Self::Inner => EffectMode::Edit,
            Self::View => EffectMode::View,
            Self::Pure => EffectMode::Pure,
        }
    }
}

impl FrameBindings {
    pub fn new(
        context_addr: ContractAddress,
        state_addr: Option<ContractAddress>,
        code_owner: Option<ContractAddress>,
        lib_table: Arc<[ContractAddress]>,
    ) -> Self {
        Self {
            context_addr,
            state_addr,
            code_owner,
            lib_table,
        }
    }

    pub fn root(context_addr: ContractAddress, lib_table: Arc<[ContractAddress]>) -> Self {
        Self::new(context_addr, None, None, lib_table)
    }

    pub fn contract(
        context_addr: ContractAddress,
        state_addr: ContractAddress,
        code_owner: ContractAddress,
        lib_table: Arc<[ContractAddress]>,
    ) -> Self {
        Self::new(context_addr, Some(state_addr), Some(code_owner), lib_table)
    }

    pub fn current_addr(&self) -> &ContractAddress {
        self.code_owner.as_ref().unwrap_or(&self.context_addr)
    }
}

impl ExecCtx {
    pub const fn new(entry: EntryKind, effect: EffectMode, call_depth: usize) -> Self {
        Self {
            entry,
            effect,
            call_depth,
        }
    }

    pub const fn main() -> Self {
        Self::new(EntryKind::Main, EffectMode::Edit, 0)
    }

    pub const fn p2sh() -> Self {
        Self::new(EntryKind::P2sh, EffectMode::Edit, 0)
    }

    pub const fn abst() -> Self {
        Self::new(EntryKind::Abst, EffectMode::Edit, 1)
    }

    pub const fn contract(entry: EntryKind, effect: EffectMode) -> Self {
        Self::new(entry, effect, 1)
    }

    pub const fn external() -> Self {
        Self::contract(EntryKind::Main, EffectMode::Edit)
    }

    pub const fn inner() -> Self {
        Self::contract(EntryKind::Main, EffectMode::Edit)
    }

    pub const fn view() -> Self {
        Self::contract(EntryKind::Main, EffectMode::View)
    }

    pub const fn pure() -> Self {
        Self::contract(EntryKind::Main, EffectMode::Pure)
    }

    pub const fn is_outer_entry(self) -> bool {
        self.call_depth == 0
    }

    pub fn ensure_call_depth(self, cap: &SpaceCap) -> VmrtErr {
        if self.call_depth > cap.call_depth {
            return itr_err_code!(OutOfCallDepth);
        }
        Ok(())
    }

    pub fn enter_call(self, effect: EffectMode, cap: &SpaceCap) -> VmrtRes<Self> {
        let next = self
            .call_depth
            .checked_add(1)
            .ok_or_else(|| ItrErr::code(OutOfCallDepth))?;
        let next = Self::new(self.entry, effect, next);
        next.ensure_call_depth(cap)?;
        Ok(next)
    }
}
