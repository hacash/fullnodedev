use std::any::*;
use std::cell::RefCell;
use std::collections::*;
use std::sync::*;

use basis::interface::*;
use sys::*;

include! {"block_hasher.rs"}
include! {"action_creater.rs"}
include! {"action_hooker.rs"}
include! {"vm_assigner.rs"}
include!{"prelude_tx.rs"}

pub struct SetupBuilder {
    block_hasher: Option<FnBlockHasherFunc>,
    vm_assigner: Option<FnVmAssignFunc>,
    prelude_tx_codec: Option<PreludeTxCodec>,
    action_registers: Vec<ActionRegisterItem>,
    action_hooks: Vec<FnActionHookFunc>,
}

pub struct SetupRegistryData {
    block_hasher: FnBlockHasherFunc,
    pub(crate) vm_assigner: Option<FnVmAssignFunc>,
    pub(crate) prelude_tx_codec: Option<PreludeTxCodec>,
    action_codecs: HashMap<u16, ActionCodec>,
    action_hooks: Vec<FnActionHookFunc>,
}

pub type SetupRegistry = Arc<SetupRegistryData>;

impl SetupBuilder {
    pub fn new() -> Self {
        Self {
            block_hasher: None,
            vm_assigner: None,
            prelude_tx_codec: None,
            action_registers: vec![],
            action_hooks: vec![],
        }
    }

    pub fn block_hasher(mut self, f: FnBlockHasherFunc) -> Self {
        self.block_hasher = Some(f);
        self
    }

    pub fn vm_assigner(mut self, f: FnVmAssignFunc) -> Self {
        self.vm_assigner = Some(f);
        self
    }

    pub fn action_register(self, register: fn(SetupBuilder) -> SetupBuilder) -> Self {
        register(self)
    }

    pub fn prelude_tx_codec(
        mut self,
        create: FnPreludeTxCreateFunc,
        json_decode: FnPreludeTxJsonDecodeFunc,
    ) -> Self {
        self.prelude_tx_codec = Some(PreludeTxCodec { create, json_decode });
        self
    }

    pub fn register_codec(
        mut self,
        kinds: &'static [u16],
        create: ActCreateFun,
        json_decode: ActJSONDecodeFun,
    ) -> Self {
        self.action_registers
            .push(ActionRegisterItem::new(kinds, create, json_decode));
        self
    }

    pub fn action_hooker(mut self, f: FnActionHookFunc) -> Self {
        self.action_hooks.push(f);
        self
    }

    pub fn build(self) -> Ret<SetupRegistry> {
        let block_hasher = self.block_hasher.unwrap_or(default_block_hasher);
        let mut action_codecs = HashMap::<u16, ActionCodec>::new();
        for (gid, reg) in self.action_registers.iter().enumerate() {
            if reg.kinds.is_empty() {
                return errf!("action register {} has empty kind list", gid);
            }
            for kind in reg.kinds {
                if action_codecs.insert(*kind, reg.codec).is_some() {
                    return errf!("action kind {} conflict in register {}", kind, gid);
                }
            }
        }
        Ok(Arc::new(SetupRegistryData {
            block_hasher,
            vm_assigner: self.vm_assigner,
            prelude_tx_codec: self.prelude_tx_codec,
            action_codecs,
            action_hooks: self.action_hooks,
        }))
    }
}

impl Default for SetupBuilder {
    fn default() -> Self {
        Self::new()
    }
}

static GLOBAL_SETUP_REGISTRY: OnceLock<SetupRegistry> = OnceLock::new();

thread_local! {
    static SCOPED_SETUP_REGISTRY: RefCell<Option<SetupRegistry>> = const { RefCell::new(None) };
}

pub struct ScopedSetupGuard {
    old: Option<SetupRegistry>,
}

impl Drop for ScopedSetupGuard {
    fn drop(&mut self) {
        let old = self.old.take();
        SCOPED_SETUP_REGISTRY.with(|cell| {
            *cell.borrow_mut() = old;
        });
    }
}

pub fn install_once(registry: SetupRegistry) -> Rerr {
    if GLOBAL_SETUP_REGISTRY.set(registry).is_err() {
        return errf!("setup registry already installed");
    }
    Ok(())
}

pub fn install_builder(builder: SetupBuilder) -> Rerr {
    install_once(builder.build()?)
}

pub fn standard_protocol_builder(block_hasher: FnBlockHasherFunc) -> SetupBuilder {
    SetupBuilder::new()
        .block_hasher(block_hasher)
        .action_register(crate::action::register)
        .action_register(crate::tex::register)
}

pub fn install_standard_protocol_stack(block_hasher: FnBlockHasherFunc) -> Rerr {
    install_builder(standard_protocol_builder(block_hasher))
}

pub fn install_scoped_for_test(registry: SetupRegistry) -> ScopedSetupGuard {
    let old = SCOPED_SETUP_REGISTRY.with(|cell| cell.replace(Some(registry)));
    ScopedSetupGuard { old }
}

pub fn get_registry() -> Ret<SetupRegistry> {
    if let Some(scoped) = SCOPED_SETUP_REGISTRY.with(|cell| cell.borrow().as_ref().cloned()) {
        return Ok(scoped);
    }
    let Some(global) = GLOBAL_SETUP_REGISTRY.get() else {
        return errf!("setup registry not installed");
    };
    Ok(global.clone())
}
