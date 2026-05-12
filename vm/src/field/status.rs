
combi_struct! { StatusKV,
    key: BytesW1
    value: Value
}
combi_list! { StatusKVList, Uint2, StatusKV }

use crate::space::{validate_scalar_payload_len, validate_volatile_scalar_put, VolatileKvLimits};

#[derive(Default, Debug, Clone, PartialEq, Eq)]
pub struct StatusSto {
    items: StatusKVList,
}

#[derive(Default, Debug, Clone, PartialEq, Eq)]
struct StatusMap {
    items: BTreeMap<Vec<u8>, Value>,
}

impl StatusMap {
    fn from_storage(sto: &StatusSto) -> VmrtRes<Self> {
        let mut map = BTreeMap::new();
        for item in sto.items.as_list() {
            let value = &item.value;
            if matches!(value, Value::Nil) {
                return itr_err_fmt!(StorageError, "status value cannot be nil in storage");
            }
            value.check_scalar()?;
            let key = item.key.to_vec();
            if key.is_empty() {
                return itr_err_fmt!(StorageError, "status key cannot be empty in storage");
            }
            if map.insert(key, value.clone()).is_some() {
                return itr_err_fmt!(StorageError, "duplicate status key in storage");
            }
        }
        Ok(Self { items: map })
    }

    fn to_storage(&self) -> Ret<StatusSto> {
        let items = self
            .items
            .iter()
            .map(|(key, value)| {
                Ok(StatusKV {
                    key: BytesW1::from(key.clone())?,
                    value: value.clone(),
                })
            })
            .collect::<Ret<Vec<_>>>()?;
        Ok(StatusSto {
            items: StatusKVList::from_list(items)?,
        })
    }

    fn payload_size(&self) -> usize {
        let mut total = 0usize;
        for (k, v) in &self.items {
            total = add_size_saturating(total, k.len());
            if total == usize::MAX {
                break;
            }
            total = add_size_saturating(total, v.val_size());
            if total == usize::MAX {
                break;
            }
        }
        total
    }

    fn validate_key_lengths(&self, key_max: usize, ec: ItrErrCode) -> VmrtErr {
        for key in self.items.keys() {
            if key.len() > key_max {
                return itr_err_fmt!(
                    ec,
                    "status key too long, max {} bytes but got {}",
                    key_max,
                    key.len()
                );
            }
        }
        Ok(())
    }

    fn ensure_save_bounds(&self, cap: &SpaceCap) -> VmrtErr {
        self.validate_key_lengths(cap.kv_key_size, StorageKeyInvalid)?;
        for (_, v) in &self.items {
            validate_volatile_scalar_put(v, cap.value_size, StorageValSizeErr)?;
        }
        let payload = self.payload_size();
        if payload > cap.status_pure_size {
            return itr_err_fmt!(
                StorageValSizeErr,
                "status payload too large, max {} bytes but got {}",
                cap.status_pure_size,
                payload
            );
        }
        Ok(())
    }

    #[inline(always)]
    fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    #[inline(always)]
    fn get(&self, key: &[u8]) -> Value {
        self.items.get(key).cloned().unwrap_or(Value::Nil)
    }

    fn set_or_remove(&mut self, key: Vec<u8>, value: Value) -> VmrtErr {
        if matches!(value, Value::Nil) {
            self.items.remove(&key);
        } else {
            value.check_scalar()?;
            self.items.insert(key, value);
        }
        Ok(())
    }
}

impl StatusSto {
    fn from_status_map(map: &StatusMap) -> Ret<Self> {
        map.to_storage()
    }

    fn to_status_map(&self) -> VmrtRes<StatusMap> {
        StatusMap::from_storage(self)
    }
}

impl Parse for StatusSto {
    fn parse_from(&mut self, buf: &mut &[u8]) -> Ret<usize> {
        self.items.parse_from(buf)
    }
}

impl Serialize for StatusSto {
    fn serialize_to(&self, out: &mut Vec<u8>) {
        self.items.serialize_to(out)
    }

    fn size(&self) -> usize {
        self.items.size()
    }
}

impl_field_only_new! { StatusSto }

impl ToJSON for StatusSto {
    fn to_json_fmt(&self, fmt: &JSONFormater) -> String {
        self.items.to_json_fmt(fmt)
    }
}

impl FromJSON for StatusSto {
    fn from_json(&mut self, json: &str) -> Ret<()> {
        self.items.from_json(json)
    }
}
