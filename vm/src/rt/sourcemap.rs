use std::cell::RefCell;
use std::collections::{HashMap, HashSet};

use hex;
use serde::{Deserialize, Serialize};
use serde_json;

use sys::*;
use field::Address;


#[derive(Serialize, Deserialize)]
struct SourceMapJson {
    libs: Vec<LibJson>,
    funcs: Vec<FuncJson>,
    slots: Vec<SlotJson>,
    puts: Vec<u8>,
}

#[derive(Serialize, Deserialize)]
struct LibJson {
    idx: u8,
    name: String,
    address: Option<String>,
}

#[derive(Serialize, Deserialize)]
struct FuncJson {
    sig: String,
    name: String,
}

#[derive(Serialize, Deserialize)]
struct SlotJson {
    slot: u8,
    name: String,
}


#[derive(Debug, Clone)]
pub struct LibInfo {
    pub name: String,
    pub address: Option<Address>,
}

#[derive(Debug, Clone)]
pub struct SourceMap {
    libs: HashMap<u8, LibInfo>,
    funcs: HashMap<FnSign, String>,
    slots: HashMap<u8, String>,
    allocated: RefCell<HashSet<u8>>,
}

impl Default for SourceMap {
    fn default() -> Self {
        Self {
            libs: HashMap::new(),
            funcs: HashMap::new(),
            slots: HashMap::new(),
            allocated: RefCell::new(HashSet::new()),
        }
    }
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

    pub fn mark_slot_put(&self, slot: u8) -> bool {
        self.allocated.borrow_mut().insert(slot)
    }

    pub fn clear_slot_put(&self, slot: u8) {
        self.allocated.borrow_mut().remove(&slot);
    }

    pub fn clear_all_slot_puts(&self) {
        self.allocated.borrow_mut().clear();
    }

    pub fn to_json(&self) -> Ret<String> {
        let libs: Vec<LibJson> = self.libs.iter().map(|(&idx, info)| LibJson {
            idx,
            name: info.name.clone(),
            address: info.address.as_ref().map(|a| a.readable()),
        }).collect();
        let funcs: Vec<FuncJson> = self.funcs.iter().map(|(sig, name)| FuncJson {
            sig: hex::encode(sig),
            name: name.clone(),
        }).collect();
        let slots: Vec<SlotJson> = self.slots.iter().map(|(&slot, name)| SlotJson {
            slot,
            name: name.clone(),
        }).collect();
        let puts: Vec<u8> = self.allocated.borrow().iter().copied().collect();
        let doc = SourceMapJson { libs, funcs, slots, puts };
        serde_json::to_string(&doc).map_err(|_|s!("source map serialize failed"))
    }

    pub fn from_json(text: &str) -> Ret<Self> {
        let doc: SourceMapJson = serde_json::from_str(text).map_err(|_|s!("source map deserialize failed"))?;
        let mut map = SourceMap::default();
        for lib in doc.libs {
            let address = match lib.address {
                Some(addr) => Some(Address::from_readable(&addr).map_err(|_|s!("address parse failed"))?),
                None => None,
            };
            map.register_lib(lib.idx, lib.name, address)?;
        }
        for func in doc.funcs {
            let bytes = hex::decode(func.sig).map_err(|_|s!("function signature decode failed"))?;
            if bytes.len() != 4 {
                return errf!("function signature wrong length")
            }
            let mut sig = [0u8; 4];
            sig.copy_from_slice(&bytes);
            map.register_func(sig, func.name)?;
        }
        for slot in doc.slots {
            map.register_slot(slot.slot, slot.name)?;
        }
        for slot in doc.puts {
            map.mark_slot_put(slot);
        }
        Ok(map)
    }
}
