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
// include!{"server_router.rs"}

pub struct SetupBuilder {
    block_hasher: Option<FnBlockHasherFunc>,
    vm_assigner: Option<FnVmAssignFunc>,
    action_registers: Vec<ActionRegisterItem>,
    action_hooks: Vec<FnActionHookFunc>,
}

pub struct SetupRegistryData {
    block_hasher: FnBlockHasherFunc,
    vm_assigner: Option<FnVmAssignFunc>,
    action_codecs: HashMap<u16, ActionCodec>,
    action_hooks: Vec<FnActionHookFunc>,
}

pub type SetupRegistry = Arc<SetupRegistryData>;

impl SetupBuilder {
    pub fn new() -> Self {
        Self {
            block_hasher: None,
            vm_assigner: None,
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

    pub fn register_codec(
        mut self,
        kinds: &'static [u16],
        create: ActCreateFun,
        json_decode: ActJSONDecodeFun,
        need_vm_runtime: bool,
    ) -> Self {
        self.action_registers.push(ActionRegisterItem::new(
            kinds,
            create,
            json_decode,
            need_vm_runtime,
        ));
        self
    }

    pub fn action_hooker(mut self, f: FnActionHookFunc) -> Self {
        self.action_hooks.push(f);
        self
    }

    pub fn build(self) -> Ret<SetupRegistry> {
        let Some(block_hasher) = self.block_hasher else {
            return errf!("setup missing block_hasher");
        };
        let mut action_codecs = HashMap::<u16, ActionCodec>::new();
        let mut vm_runtime_required = false;
        for (gid, reg) in self.action_registers.iter().enumerate() {
            if reg.kinds.is_empty() {
                return errf!("action register {} has empty kind list", gid);
            }
            if reg.need_vm_runtime {
                vm_runtime_required = true;
            }
            for kind in reg.kinds {
                if action_codecs.insert(*kind, reg.codec).is_some() {
                    return errf!("action kind {} conflict in register {}", kind, gid);
                }
            }
        }
        if vm_runtime_required {
            if self.vm_assigner.is_none() {
                return errf!("vm runtime actions registered but vm_assigner missing");
            }
            if self.action_hooks.is_empty() {
                return errf!("vm runtime actions registered but action_hooker missing");
            }
        }
        Ok(Arc::new(SetupRegistryData {
            block_hasher,
            vm_assigner: self.vm_assigner,
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
