use std::collections::HashMap;
use std::collections::hash_map::Entry;

use field::Address;

use crate::rt::ItrErrCode::*;
use crate::rt::*;
use crate::value::Value;

macro_rules! memories_kvmap_define {
    ($class:ident, $er1:expr, $er2:expr) => {
        #[allow(dead_code)]
        #[derive(Default, Clone, Debug)]
        pub struct $class {
            limit: usize,
            datas: HashMap<Vec<u8>, Value>,
        }

        impl $class {
            pub fn new(limit: usize) -> Self {
                Self {
                    limit,
                    datas: HashMap::new(),
                }
            }

            pub fn reset(&mut self, lmt: usize) {
                self.limit = lmt;
                self.clear();
            }

            pub fn clear(&mut self) {
                self.datas.clear();
            }

            fn key(k: &Value) -> VmrtRes<Vec<u8>> {
                let key = k.extract_key_bytes()?;
                if key.is_empty() {
                    return itr_err_fmt!($er1, "key {} cannot be empty", k);
                }
                Ok(key)
            }

            pub fn put(&mut self, k: Value, v: Value) -> VmrtErr {
                self.put_with_stats(k, v).map(|_| ())
            }

            pub fn put_with_stats(&mut self, k: Value, v: Value) -> VmrtRes<(usize, bool)> {
                v.check_scalar()?;
                let key = Self::key(&k)?;
                let key_len = key.len();
                let full = self.datas.len() >= self.limit;
                match self.datas.entry(key) {
                    Entry::Occupied(mut hit) => {
                        hit.insert(v);
                        Ok((key_len, false))
                    }
                    Entry::Vacant(slot) => {
                        if full {
                            return itr_err_code!($er2); // out of limit
                        }
                        slot.insert(v);
                        Ok((key_len, true))
                    }
                }
            }

            pub fn get(&self, k: &Value) -> VmrtRes<Value> {
                Ok(match self.datas.get(&Self::key(k)?) {
                    Some(v) => v.clone(),
                    None => Value::Nil,
                })
            }

            pub fn remove(&mut self, k: &Value) -> VmrtErr {
                self.datas.remove(&Self::key(k)?);
                Ok(())
            }

            pub fn contains_key(&self, k: &Value) -> VmrtRes<bool> {
                Ok(self.datas.contains_key(&Self::key(k)?))
            }

            pub fn len(&self) -> usize {
                self.datas.len()
            }

            pub fn keys_sorted(&self) -> Vec<Vec<u8>> {
                let mut keys = self.datas.keys().cloned().collect::<Vec<_>>();
                keys.sort_unstable();
                keys
            }
        }
    };
}

/*  */
memories_kvmap_define! { GKVMap, GlobalError, OutOfGlobal }
memories_kvmap_define! { MKVMap, MemoryError, OutOfMemory }

/*  */

#[derive(Default, Clone)]
pub struct CtcKVMap {
    limit: usize,
    datas: HashMap<Address, MKVMap>,
}

impl CtcKVMap {
    #[inline(always)]
    fn check_addr(addr: &Address) -> VmrtErr {
        addr.check_version().map_ires(
            MemoryError,
            format!("memory use must be in effective address but in {}", addr),
        )
    }

    pub fn new(limit: usize) -> Self {
        Self {
            limit,
            datas: HashMap::new(),
        }
    }

    pub fn reset(&mut self, lmt: usize) {
        self.limit = lmt;
        self.clear();
    }

    pub fn clear(&mut self) {
        self.datas.clear();
    }

    pub fn entry_mut(&mut self, addr: &Address) -> VmrtRes<&mut MKVMap> {
        Self::check_addr(addr)?;
        Ok(self
            .datas
            .entry(addr.clone())
            .or_insert_with(|| MKVMap::new(self.limit)))
    }

    pub fn get(&self, addr: &Address, key: &Value) -> VmrtRes<Value> {
        Self::check_addr(addr)?;
        match self.datas.get(addr) {
            Some(mem) => mem.get(key),
            None => Ok(Value::Nil),
        }
    }

    pub fn remove(&mut self, addr: &Address, key: &Value) -> VmrtErr {
        Self::check_addr(addr)?;
        if let Some(mem) = self.datas.get_mut(addr) {
            mem.remove(key)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod kvmap_tests {
    use super::*;

    #[test]
    fn ctc_reset_clears_datas_and_uses_new_limit() {
        let addr = Address::zero();
        let key = Value::Bytes(vec![1u8]);
        let mut m = CtcKVMap::new(1);

        m.entry_mut(&addr)
            .unwrap()
            .put(key.clone(), Value::U8(7))
            .unwrap();
        m.reset(0);

        let got = m.get(&addr, &key).unwrap();
        assert_eq!(got, Value::Nil);

        let err = m
            .entry_mut(&addr)
            .unwrap()
            .put(key.clone(), Value::U8(9))
            .unwrap_err();
        assert!(matches!(err, ItrErr(OutOfMemory, _)));
    }
}
