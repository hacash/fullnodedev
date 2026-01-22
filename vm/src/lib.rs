#[macro_use]
pub mod rt;
pub mod action;
pub mod contract;
pub mod frame;
pub mod hook;
pub mod interpreter;
pub mod ir;
pub mod lang;
pub mod machine;
pub mod native;
pub mod space;
pub mod value;

pub use rt::VmrtRes;

use machine::*;

include! {"field/mod.rs"}
include! {"interface/mod.rs"}

use std::sync::OnceLock;
static MACHINE_MANAGER_INSTANCE: OnceLock<MachineManage> = OnceLock::new();

pub fn global_machine_manager() -> &'static MachineManage {
    MACHINE_MANAGER_INSTANCE.get_or_init(|| MachineManage::new())
}
