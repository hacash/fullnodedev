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
    lets: Vec<u8>,
    vars: Vec<u8>,
    params: Vec<String>,
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
    kind: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SlotKind {
    Param,
    Var,
    Let,
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
    slots: HashMap<u8, SlotInfo>,
    params: Vec<String>,
    lets: HashSet<u8>,
    vars: HashSet<u8>,
}

#[derive(Debug, Clone)]
struct SlotInfo {
    name: String,
    kind: SlotKind,
}

impl Default for SourceMap {
    fn default() -> Self {
        Self {
            libs: HashMap::new(),
            funcs: HashMap::new(),
            slots: HashMap::new(),
            params: Vec::new(),
            lets: HashSet::new(),
            vars: HashSet::new(),
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

    pub fn register_slot(&mut self, slot: u8, name: String, kind: SlotKind) -> Rerr {
        self.slots.insert(slot, SlotInfo { name, kind });
        match kind {
            SlotKind::Var | SlotKind::Let => {
                self.vars.remove(&slot);
                self.lets.insert(slot);
            }
            SlotKind::Param => {}
        }
        Ok(())
    }

    pub fn register_param_names(&mut self, names: Vec<String>) -> Rerr {
        self.params = names;
        Ok(())
    }

    pub fn param_names(&self) -> Option<&Vec<String>> {
        if self.params.is_empty() {
            None
        } else {
            Some(&self.params)
        }
    }

    pub fn lib(&self, idx: u8) -> Option<&LibInfo> {
        self.libs.get(&idx)
    }

    pub fn func(&self, sig: &[u8; 4]) -> Option<&String> {
        self.funcs.get(sig)
    }

    pub fn slot(&self, slot: u8) -> Option<&String> {
        self.slots.get(&slot).map(|info| &info.name)
    }

    pub fn lib_entries(&self) -> Vec<(u8, LibInfo)> {
        let mut libs: Vec<(u8, LibInfo)> = self.libs.iter().map(|(&idx, info)| (idx, info.clone())).collect();
        libs.sort_by_key(|(idx, _)| *idx);
        libs
    }

    pub fn mark_slot_mutated(&mut self, slot: u8) {
        if self.vars.contains(&slot) {
            return;
        }
        self.lets.remove(&slot);
        self.vars.insert(slot);
    }

    pub fn slot_is_var(&self, slot: u8) -> bool {
        self.vars.contains(&slot)
    }

    pub fn slot_is_let(&self, slot: u8) -> bool {
        self.lets.contains(&slot)
    }

    pub fn to_json(&self) -> Ret<String> {
        let mut libs: Vec<LibJson> = self.libs.iter().map(|(&idx, info)| LibJson {
            idx,
            name: info.name.clone(),
            address: info.address.as_ref().map(|a| a.readable()),
        }).collect();
        libs.sort_by_key(|entry| entry.idx);

        let mut funcs: Vec<FuncJson> = self.funcs.iter().map(|(sig, name)| FuncJson {
            sig: hex::encode(sig),
            name: name.clone(),
        }).collect();
        funcs.sort_by(|a, b| a.sig.cmp(&b.sig));

        let mut slots: Vec<SlotJson> = self.slots.iter().map(|(&slot, info)| SlotJson {
            slot,
            name: info.name.clone(),
            kind: slot_kind_to_str(info.kind).to_owned(),
        }).collect();
        slots.sort_by_key(|entry| entry.slot);

        let mut lets: Vec<u8> = self.lets.iter().copied().collect();
        lets.sort_unstable();
        let mut vars: Vec<u8> = self.vars.iter().copied().collect();
        vars.sort_unstable();

        let doc = SourceMapJson { libs, funcs, slots, lets, vars, params: self.params.clone() };
        serde_json::to_string(&doc).map_err(|_| s!("source map serialize failed"))
    }

    pub fn from_json(text: &str) -> Ret<Self> {
        let doc: SourceMapJson = serde_json::from_str(text).map_err(|_| s!("source map deserialize failed"))?;
        let mut map = SourceMap::default();
        for lib in doc.libs {
            let address = match lib.address {
                Some(addr) => Some(Address::from_readable(&addr).map_err(|_| s!("address parse failed"))?),
                None => None,
            };
            map.register_lib(lib.idx, lib.name, address)?;
        }
        for func in doc.funcs {
            let bytes = hex::decode(func.sig).map_err(|_| s!("function signature decode failed"))?;
            if bytes.len() != 4 {
                return errf!("function signature wrong length")
            }
            let mut sig = [0u8; 4];
            sig.copy_from_slice(&bytes);
            map.register_func(sig, func.name)?;
        }
        for slot in doc.slots {
            let kind = slot_kind_from_str(&slot.kind)?;
            map.register_slot(slot.slot, slot.name, kind)?;
        }
        map.lets.clear();
        map.vars.clear();
        for slot in doc.lets {
            map.lets.insert(slot);
        }
        for slot in doc.vars {
            map.vars.insert(slot);
            map.lets.remove(&slot);
        }
        map.params = doc.params;
        Ok(map)
    }
}

fn slot_kind_to_str(kind: SlotKind) -> &'static str {
    match kind {
        SlotKind::Param => "param",
        SlotKind::Var => "var",
        SlotKind::Let => "let",
    }
}

fn slot_kind_from_str(kind: &str) -> Ret<SlotKind> {
    match kind.to_ascii_lowercase().as_str() {
        "param" => Ok(SlotKind::Param),
        "var" => Ok(SlotKind::Var),
        "let" => Ok(SlotKind::Let),
        _ => errf!("slot kind '{}' unsupported", kind),
    }
}
