use std::collections::HashMap;

use sys::*;
use field::Address;

#[derive(Debug, Clone)]
pub struct LibInfo {
    pub name: String,
    pub address: Option<Address>,
}

#[derive(Debug, Clone, Default)]
pub struct SourceMap {
    libs: HashMap<u8, LibInfo>,
    funcs: HashMap<FnSign, String>,
    slots: HashMap<u8, String>,
}

impl SourceMap {
    pub fn register_lib(&mut self, idx: u8, name: String, address: Option<Address>) -> Rerr {
        self.libs.insert(idx, LibInfo { name, address });
        Ok(())
    }

    pub fn register_func(&mut self, sig: [u8; 4], name: String) -> Rerr {
        self.funcs.insert(sig, name);
        Ok(())
    }

    pub fn register_slot(&mut self, slot: u8, name: String) -> Rerr {
        self.slots.insert(slot, name);
        Ok(())
    }

    pub fn lib(&self, idx: u8) -> Option<&LibInfo> {
        self.libs.get(&idx)
    }

    pub fn func(&self, sig: &[u8; 4]) -> Option<&String> {
        self.funcs.get(sig)
    }

    pub fn slot(&self, slot: u8) -> Option<&String> {
        self.slots.get(&slot)
    }
}
