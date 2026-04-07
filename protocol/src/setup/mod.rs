use std::any::Any;
use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::{Arc, OnceLock};

use basis::interface::*;
use sys::*;

include! {"block_hasher.rs"}
include! {"action_creater.rs"}
include! {"action_hooker.rs"}
include! {"vm_assigner.rs"}
include! {"tx_codec.rs"}

pub struct ProtocolSetup {
    block_hasher: FnBlockHasherFunc,
    pub(crate) vm_assigner: Option<FnVmAssignFunc>,
    pub(crate) tx_codecs: HashMap<u8, TxCodec>,
    action_codecs: HashMap<u16, ActionCodec>,
    action_hooks: Vec<FnActionHookFunc>,
}

impl ProtocolSetup {
    pub fn new(block_hasher: FnBlockHasherFunc) -> Self {
        Self {
            block_hasher,
            vm_assigner: None,
            tx_codecs: HashMap::new(),
            action_codecs: HashMap::new(),
            action_hooks: vec![],
        }
    }

    pub fn set_block_hasher(&mut self, f: FnBlockHasherFunc) {
        self.block_hasher = f;
    }

    pub fn set_vm_assigner(&mut self, f: FnVmAssignFunc) {
        self.vm_assigner = Some(f);
    }

    pub fn tx_codec(
        &mut self,
        ty: u8,
        create: FnTxCreateFunc,
        json_decode: FnTxJsonDecodeFunc,
    ) {
        self.tx_codecs.insert(
            ty,
            TxCodec {
                create,
                json_decode,
            },
        );
    }

    pub fn action_codec(
        &mut self,
        kinds: &'static [u16],
        create: ActCreateFun,
        json_decode: ActJSONDecodeFun,
    ) {
        let codec = ActionCodec {
            create,
            json_decode,
        };
        for kind in kinds {
            self.action_codecs.insert(*kind, codec);
        }
    }

    pub fn action_hook(&mut self, f: FnActionHookFunc) {
        self.action_hooks.push(f);
    }

}

impl Default for ProtocolSetup {
    fn default() -> Self {
        Self::new(default_block_hasher)
    }
}

static GLOBAL_SETUP_REGISTRY: OnceLock<Arc<ProtocolSetup>> = OnceLock::new();

thread_local! {
    static SCOPED_SETUP_REGISTRY: RefCell<Option<Arc<ProtocolSetup>>> = const { RefCell::new(None) };
}

pub struct TestSetupScopeGuard {
    old: Option<Arc<ProtocolSetup>>,
}

impl Drop for TestSetupScopeGuard {
    fn drop(&mut self) {
        let old = self.old.take();
        SCOPED_SETUP_REGISTRY.with(|cell| {
            *cell.borrow_mut() = old;
        });
    }
}

pub fn install_once(registry: ProtocolSetup) -> Rerr {
    if GLOBAL_SETUP_REGISTRY.set(Arc::new(registry)).is_err() {
        return errf!("setup registry already installed");
    }
    Ok(())
}

pub fn install_test_scope(registry: ProtocolSetup) -> TestSetupScopeGuard {
    let old = SCOPED_SETUP_REGISTRY.with(|cell| cell.replace(Some(Arc::new(registry))));
    TestSetupScopeGuard { old }
}

pub fn current_setup() -> Ret<Arc<ProtocolSetup>> {
    if let Some(scoped) = SCOPED_SETUP_REGISTRY.with(|cell| cell.borrow().as_ref().cloned()) {
        return Ok(scoped);
    }
    let Some(global) = GLOBAL_SETUP_REGISTRY.get() else {
        return errf!("setup registry not installed");
    };
    Ok(global.clone())
}

pub fn new_standard_protocol_setup(block_hasher: FnBlockHasherFunc) -> ProtocolSetup {
    let mut setup = ProtocolSetup::new(block_hasher);
    crate::transaction::register(&mut setup);
    crate::action::register(&mut setup);
    crate::tex::register(&mut setup);
    setup
}

