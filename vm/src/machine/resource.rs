/// Block heights at which VM protocol upgrades activate.
/// Append new heights here for hard forks that change GasTable/GasExtra/SpaceCap.
/// Must be sorted in ascending order.
const UPGRADE_HEIGHTS: &[u64] = &[
    // 200000,  // example: v1 adjustments
];

#[derive(Clone, Debug)]
pub struct IntentEntry {
    pub kind: Vec<u8>,
    pub data: MKVMap,
}

#[derive(Clone, Debug, Default)]
struct IntentBucketMap {
    datas: HashMap<Address, HashMap<usize, IntentEntry>>,
}

impl IntentBucketMap {
    fn clear(&mut self) {
        self.datas.clear();
    }

    fn entry_mut(&mut self, owner: &ContractAddress) -> &mut HashMap<usize, IntentEntry> {
        self.datas.entry(owner.to_addr()).or_default()
    }

    fn get(&self, owner: &ContractAddress) -> Option<&HashMap<usize, IntentEntry>> {
        self.datas.get(&owner.to_addr())
    }

    fn get_mut(&mut self, owner: &ContractAddress) -> Option<&mut HashMap<usize, IntentEntry>> {
        self.datas.get_mut(&owner.to_addr())
    }

    fn remove(&mut self, owner: &ContractAddress, id: usize) -> Option<IntentEntry> {
        self.datas
            .get_mut(&owner.to_addr())
            .and_then(|m| m.remove(&id))
    }
}

#[derive(Clone, Debug)]
pub struct IntentRuntime {
    next_id: usize,
    id_generation: usize,
    total_created: usize,
    create_limit: usize,
    key_limit: usize,
    val_size_limit: usize,
    intent_key_limit: usize,
    owners: HashMap<usize, ContractAddress>,
    buckets: IntentBucketMap,
}

impl Default for IntentRuntime {
    fn default() -> Self {
        Self::new(0, 0, 0, 0)
    }
}

impl IntentRuntime {
    pub fn new(create_limit: usize, key_limit: usize, val_size_limit: usize, intent_key_limit: usize) -> Self {
        Self {
            next_id: 0,
            id_generation: 0,
            total_created: 0,
            create_limit,
            key_limit,
            val_size_limit,
            intent_key_limit,
            owners: HashMap::new(),
            buckets: IntentBucketMap::default(),
        }
    }

    pub fn clear(&mut self) {
        self.next_id = 0;
        self.total_created = 0;
        self.owners.clear();
        self.buckets.clear();
        // id_generation is NOT reset to prevent ID reuse
    }

    pub fn reset(&mut self, create_limit: usize, key_limit: usize, val_size_limit: usize, intent_key_limit: usize) {
        self.create_limit = create_limit;
        self.key_limit = key_limit;
        self.val_size_limit = val_size_limit;
        self.intent_key_limit = intent_key_limit;
        self.clear();
    }

    pub fn create(&mut self, owner: ContractAddress, kind: Vec<u8>) -> VmrtRes<usize> {
        if kind.is_empty() {
            return itr_err_fmt!(ItrErrCode::IntentError, "intent kind cannot be empty");
        }
        self.check_size_limit(kind.len(), "kind")?;
        if self.total_created >= self.create_limit {
            return itr_err_fmt!(
                ItrErrCode::IntentError,
                "intent creation limit {} exceeded",
                self.create_limit
            );
        }
        let next_gen = self
            .id_generation
            .checked_add(1)
            .ok_or_else(|| ItrErr::new(ItrErrCode::IntentError, "intent id generation overflow"))?;
        self.id_generation = next_gen;
        self.next_id = next_gen;
        self.total_created += 1;
        self.owners.insert(next_gen, owner.clone());
        self.buckets.entry_mut(&owner).insert(
            next_gen,
            IntentEntry {
                kind,
                data: MKVMap::new(self.key_limit),
            },
        );
        Ok(next_gen)
    }

    // Error helpers
    fn intent_not_found(id: usize) -> ItrErr {
        ItrErr::new(ItrErrCode::IntentError, &format!("intent {} not found", id))
    }

    fn key_not_found() -> ItrErr {
        ItrErr::new(ItrErrCode::IntentError, "intent key not found")
    }

    fn check_size_limit(&self, size: usize, label: &str) -> VmrtErr {
        if size > self.val_size_limit {
            return itr_err_fmt!(
                ItrErrCode::IntentError,
                "intent {} size {} exceeds limit {}",
                label,
                size,
                self.val_size_limit
            );
        }
        Ok(())
    }

    // Owner validation
    pub fn ensure_owner(&self, owner: &ContractAddress, id: usize) -> VmrtErr {
        let real = self.owner_of(id)?;
        if real != *owner {
            return itr_err_fmt!(
                ItrErrCode::IntentError,
                "intent {} is not owned by contract {}",
                id,
                owner.to_readable()
            );
        }
        Ok(())
    }

    fn require_ref(&self, owner: &ContractAddress, id: usize) -> VmrtRes<&IntentEntry> {
        self.ensure_owner(owner, id)?;
        self.buckets
            .get(owner)
            .and_then(|bucket| bucket.get(&id))
            .ok_or_else(|| Self::intent_not_found(id))
    }

    fn require_mut(&mut self, owner: &ContractAddress, id: usize) -> VmrtRes<&mut IntentEntry> {
        self.ensure_owner(owner, id)?;
        self.buckets
            .get_mut(owner)
            .and_then(|bucket| bucket.get_mut(&id))
            .ok_or_else(|| Self::intent_not_found(id))
    }

    fn validate_non_nil_scalar(val: &Value) -> VmrtErr {
        if val.is_nil() {
            return itr_err_fmt!(ItrErrCode::IntentError, "intent value cannot be nil");
        }
        val.check_non_nil_scalar(ItrErrCode::IntentError)
    }

    fn extract_intent_key_bytes(&self, key: &Value) -> VmrtRes<Vec<u8>> {
        let key_bytes = key
            .extract_key_bytes()
            .map_err(|ItrErr(_, msg)| ItrErr::new(ItrErrCode::IntentError, &msg))?;
        if key_bytes.is_empty() {
            return itr_err_fmt!(ItrErrCode::IntentError, "intent key cannot be empty");
        }
        self.check_size_limit(key_bytes.len(), "key")?;
        Ok(key_bytes)
    }

    fn validate_intent_key(&self, key: &Value) -> VmrtErr {
        self.extract_intent_key_bytes(key)?;
        Ok(())
    }

    fn validate_key_value_for_put(&self, key: &Value, val: &Value) -> VmrtErr {
        Self::validate_non_nil_scalar(val)?;
        self.validate_intent_key(key)?;
        let val_len = val.extract_bytes_len_with_error_code(ItrErrCode::IntentError)?;
        self.check_size_limit(val_len, "value")?;
        Ok(())
    }

    fn validate_uint(value: &Value, msg: &str) -> VmrtErr {
        if !value.is_uint() {
            return itr_err_fmt!(ItrErrCode::IntentError, "{}", msg);
        }
        Ok(())
    }

    fn cast_uint_pair_same_width(left: &Value, right: &Value) -> VmrtRes<(Value, Value)> {
        let lb = left
            .ty()
            .uint_bits()
            .ok_or_else(|| ItrErr::new(ItrErrCode::IntentError, "intent value must be uint"))?;
        let rb = right
            .ty()
            .uint_bits()
            .ok_or_else(|| ItrErr::new(ItrErrCode::IntentError, "intent delta must be uint"))?;
        let width = lb.max(rb);
        let mut lx = left.clone();
        let mut ry = right.clone();
        lx.cast_to_uint_width(width)
            .map_err(|ItrErr(_, msg)| ItrErr::new(ItrErrCode::IntentError, &msg))?;
        ry.cast_to_uint_width(width)
            .map_err(|ItrErr(_, msg)| ItrErr::new(ItrErrCode::IntentError, &msg))?;
        Ok((lx, ry))
    }

    fn uint_add_checked_with_msg(left: &Value, right: &Value, msg: &str) -> VmrtRes<Value> {
        let (lx, ry) = Self::cast_uint_pair_same_width(left, right)?;
        match (lx, ry) {
            (Value::U8(a), Value::U8(b)) => a.checked_add(b).map(Value::U8),
            (Value::U16(a), Value::U16(b)) => a.checked_add(b).map(Value::U16),
            (Value::U32(a), Value::U32(b)) => a.checked_add(b).map(Value::U32),
            (Value::U64(a), Value::U64(b)) => a.checked_add(b).map(Value::U64),
            (Value::U128(a), Value::U128(b)) => a.checked_add(b).map(Value::U128),
            _ => None,
        }
        .ok_or_else(|| ItrErr::new(ItrErrCode::IntentError, msg))
    }

    fn uint_sub_checked(left: &Value, right: &Value) -> VmrtRes<Value> {
        let (lx, ry) = Self::cast_uint_pair_same_width(left, right)?;
        match (lx, ry) {
            (Value::U8(a), Value::U8(b)) => a.checked_sub(b).map(Value::U8),
            (Value::U16(a), Value::U16(b)) => a.checked_sub(b).map(Value::U16),
            (Value::U32(a), Value::U32(b)) => a.checked_sub(b).map(Value::U32),
            (Value::U64(a), Value::U64(b)) => a.checked_sub(b).map(Value::U64),
            (Value::U128(a), Value::U128(b)) => a.checked_sub(b).map(Value::U128),
            _ => None,
        }
        .ok_or_else(|| ItrErr::new(ItrErrCode::IntentError, "intent sub underflow"))
    }

    fn ensure_insert_capacity(
        entry: &IntentEntry,
        key: &Value,
        intent_key_limit: usize,
    ) -> VmrtRes<bool> {
        let exists = entry.data.contains_key(key)?;
        if !exists && entry.data.len() >= intent_key_limit {
            return itr_err_fmt!(
                ItrErrCode::IntentError,
                "intent key count {} exceeds limit {}",
                entry.data.len(),
                intent_key_limit
            );
        }
        Ok(exists)
    }

    fn add_core(
        &mut self,
        owner: &ContractAddress,
        id: usize,
        key: Value,
        delta: Value,
        missing_base: Option<Value>,
        delta_err: &str,
        target_err: &str,
        overflow_err: &str,
    ) -> VmrtRes<Value> {
        Self::validate_uint(&delta, delta_err)?;
        self.validate_intent_key(&key)?;
        let base = {
            let entry = self.require_ref(owner, id)?;
            if entry.data.contains_key(&key)? {
                let existing = entry.data.get(&key)?;
                Self::validate_uint(&existing, target_err)?;
                existing
            } else {
                missing_base.ok_or_else(Self::key_not_found)?
            }
        };
        let val = Self::uint_add_checked_with_msg(&base, &delta, overflow_err)?;
        self.put(owner, id, key, val.clone())?;
        Ok(val)
    }

    fn ensure_unique_batch_keys(&self, keys: &[Value], op: &str) -> VmrtErr {
        let mut uniq = HashSet::new();
        for key in keys {
            let key_bytes = self.extract_intent_key_bytes(key)?;
            if !uniq.insert(key_bytes) {
                return itr_err_fmt!(
                    ItrErrCode::IntentError,
                    "intent {} duplicate key in batch",
                    op
                );
            }
        }
        Ok(())
    }

    pub fn put(&mut self, owner: &ContractAddress, id: usize, key: Value, val: Value) -> VmrtErr {
        self.validate_key_value_for_put(&key, &val)?;
        let intent_key_limit = self.intent_key_limit;
        let entry = self.require_mut(owner, id)?;
        Self::ensure_insert_capacity(entry, &key, intent_key_limit)?;
        entry.data.put(key, val)
    }

    pub fn exists(&self, id: usize) -> bool {
        self.owners.contains_key(&id)
    }

    pub fn owner_of(&self, id: usize) -> VmrtRes<ContractAddress> {
        self.owners
            .get(&id)
            .cloned()
            .ok_or_else(|| Self::intent_not_found(id))
    }

    pub fn is_owner(&self, owner: &ContractAddress, id: usize) -> VmrtRes<bool> {
        Ok(self.owner_of(id)? == *owner)
    }

    pub fn kind(&self, owner: &ContractAddress, id: usize) -> VmrtRes<Value> {
        Ok(Value::Bytes(self.require_ref(owner, id)?.kind.clone()))
    }

    pub fn kind_is(&self, owner: &ContractAddress, id: usize, kind: &[u8]) -> VmrtRes<bool> {
        Ok(self.require_ref(owner, id)?.kind == kind)
    }

    pub fn get(&self, owner: &ContractAddress, id: usize, key: &Value) -> VmrtRes<Value> {
        self.validate_intent_key(key)?;
        self.require_ref(owner, id)?.data.get(key)
    }

    pub fn take(&mut self, owner: &ContractAddress, id: usize, key: &Value) -> VmrtRes<Value> {
        let val = self.get(owner, id, key)?;
        if val.is_nil() {
            return Err(Self::key_not_found());
        }
        self.require_mut(owner, id)?.data.remove(key)?;
        Ok(val)
    }

    pub fn del(&mut self, owner: &ContractAddress, id: usize, key: &Value) -> VmrtErr {
        self.validate_intent_key(key)?;
        let entry = self.require_mut(owner, id)?;
        if !entry.data.contains_key(key)? {
            return Err(Self::key_not_found());
        }
        entry.data.remove(key)
    }

    pub fn has(&self, owner: &ContractAddress, id: usize, key: &Value) -> VmrtRes<bool> {
        self.validate_intent_key(key)?;
        self.require_ref(owner, id)?.data.contains_key(key)
    }

    pub fn clear_data(&mut self, owner: &ContractAddress, id: usize) -> VmrtErr {
        self.require_mut(owner, id)?.data.clear();
        Ok(())
    }

    pub fn len(&self, owner: &ContractAddress, id: usize) -> VmrtRes<usize> {
        Ok(self.require_ref(owner, id)?.data.len())
    }

    pub fn keys_sorted(&self, owner: &ContractAddress, id: usize) -> VmrtRes<Vec<Vec<u8>>> {
        Ok(self.require_ref(owner, id)?.data.keys_sorted())
    }

    pub fn get_or(
        &self,
        owner: &ContractAddress,
        id: usize,
        key: &Value,
        def: Value,
    ) -> VmrtRes<Value> {
        self.validate_intent_key(key)?;
        let entry = self.require_ref(owner, id)?;
        if entry.data.contains_key(key)? {
            entry.data.get(key)
        } else {
            Ok(def)
        }
    }

    pub fn require(&self, owner: &ContractAddress, id: usize, key: &Value) -> VmrtRes<Value> {
        let val = self.get(owner, id, key)?;
        if val.is_nil() {
            return Err(Self::key_not_found());
        }
        Ok(val)
    }

    pub fn require_eq(
        &self,
        owner: &ContractAddress,
        id: usize,
        key: &Value,
        expected: &Value,
    ) -> VmrtRes<Value> {
        Self::validate_non_nil_scalar(expected)?;
        let val = self.require(owner, id, key)?;
        if &val != expected {
            return itr_err_fmt!(ItrErrCode::IntentError, "intent value mismatch");
        }
        Ok(val)
    }

    pub fn require_absent(&self, owner: &ContractAddress, id: usize, key: &Value) -> VmrtErr {
        if self.has(owner, id, key)? {
            return itr_err_fmt!(ItrErrCode::IntentError, "intent key already exists");
        }
        Ok(())
    }

    pub fn replace(
        &mut self,
        owner: &ContractAddress,
        id: usize,
        key: Value,
        val: Value,
    ) -> VmrtRes<Value> {
        let old = self.require(owner, id, &key)?;
        self.put(owner, id, key, val)?;
        Ok(old)
    }

    pub fn destroy(&mut self, owner: &ContractAddress, id: usize) -> VmrtErr {
        self.ensure_owner(owner, id)?;
        self.owners.remove(&id);
        self.buckets.remove(owner, id);
        Ok(())
    }

    pub fn append(
        &mut self,
        owner: &ContractAddress,
        id: usize,
        key: Value,
        val: &Value,
    ) -> VmrtRes<usize> {
        self.validate_intent_key(&key)?;
        let new_bytes = match val {
            Value::Bytes(buf) => buf.clone(),
            _ => return itr_err_fmt!(ItrErrCode::IntentError, "intent append value must be Bytes"),
        };
        let mut buf = {
            let entry = self.require_ref(owner, id)?;
            if !entry.data.contains_key(&key)? {
                return Err(Self::key_not_found());
            }
            match entry.data.get(&key)? {
                Value::Bytes(buf) => buf,
                _ => return itr_err_fmt!(ItrErrCode::IntentError, "intent append target must be Bytes"),
            }
        };
        buf.extend_from_slice(&new_bytes);
        self.check_size_limit(buf.len(), "appended value")?;
        self.put(owner, id, key, Value::Bytes(buf.clone()))?;
        Ok(buf.len())
    }

    pub fn add(
        &mut self,
        owner: &ContractAddress,
        id: usize,
        key: Value,
        delta: Value,
    ) -> VmrtRes<Value> {
        self.add_core(
            owner,
            id,
            key,
            delta,
            None,
            "intent add delta must be uint",
            "intent add target must be uint",
            "intent add overflow",
        )
    }

    pub fn sub(
        &mut self,
        owner: &ContractAddress,
        id: usize,
        key: Value,
        delta: Value,
    ) -> VmrtRes<Value> {
        Self::validate_uint(&delta, "intent sub delta must be uint")?;
        self.validate_intent_key(&key)?;
        let entry = self.require_mut(owner, id)?;
        if !entry.data.contains_key(&key)? {
            return Err(Self::key_not_found());
        }
        let existing = entry.data.get(&key)?;
        Self::validate_uint(&existing, "intent sub target must be uint")?;
        let val = Self::uint_sub_checked(&existing, &delta)?;
        entry.data.put(key, val.clone())?;
        Ok(val)
    }

    pub fn inc(
        &mut self,
        owner: &ContractAddress,
        id: usize,
        key: Value,
        delta: Value,
    ) -> VmrtRes<Value> {
        self.add_core(
            owner,
            id,
            key,
            delta,
            Some(Value::U64(0)),
            "intent inc delta must be uint",
            "intent inc target must be uint",
            "intent inc overflow",
        )
    }

    pub fn put_if_absent(
        &mut self,
        owner: &ContractAddress,
        id: usize,
        key: Value,
        val: Value,
    ) -> VmrtRes<bool> {
        self.validate_key_value_for_put(&key, &val)?;
        let intent_key_limit = self.intent_key_limit;
        let entry = self.require_mut(owner, id)?;
        if Self::ensure_insert_capacity(entry, &key, intent_key_limit)? {
            return Ok(false);
        }
        entry.data.put(key, val)?;
        Ok(true)
    }

    // Core conditional operation: check if key exists and matches expected value
    fn conditional_op_core(
        &mut self,
        owner: &ContractAddress,
        id: usize,
        key: &Value,
        expected: &Value,
    ) -> VmrtRes<Option<Value>> {
        Self::validate_non_nil_scalar(expected)?;
        self.validate_intent_key(key)?;
        let entry = self.require_mut(owner, id)?;
        let existing = entry.data.get(key)?;
        if existing.is_nil() {
            return Err(Self::key_not_found());
        }
        if existing != *expected {
            return Ok(None); // Mismatch
        }
        Ok(Some(existing)) // Match
    }

    pub fn replace_if(
        &mut self,
        owner: &ContractAddress,
        id: usize,
        key: Value,
        old_val: Value,
        new_val: Value,
    ) -> VmrtRes<bool> {
        Self::validate_non_nil_scalar(&new_val)?;
        let new_val_len = new_val.extract_bytes_len_with_error_code(ItrErrCode::IntentError)?;
        self.check_size_limit(new_val_len, "value")?;

        match self.conditional_op_core(owner, id, &key, &old_val)? {
            None => Ok(false),
            Some(_) => {
                self.require_mut(owner, id)?.data.put(key, new_val)?;
                Ok(true)
            }
        }
    }

    pub fn del_if(
        &mut self,
        owner: &ContractAddress,
        id: usize,
        key: Value,
        old_val: Value,
    ) -> VmrtRes<bool> {
        match self.conditional_op_core(owner, id, &key, &old_val)? {
            None => Ok(false),
            Some(_) => {
                self.require_mut(owner, id)?.data.remove(&key)?;
                Ok(true)
            }
        }
    }

    pub fn take_if(
        &mut self,
        owner: &ContractAddress,
        id: usize,
        key: Value,
        old_val: Value,
    ) -> VmrtRes<(bool, Value)> {
        match self.conditional_op_core(owner, id, &key, &old_val)? {
            None => {
                // Mismatch: return false and current value
                let existing = self.require_ref(owner, id)?.data.get(&key)?;
                Ok((false, existing))
            }
            Some(val) => {
                self.require_mut(owner, id)?.data.remove(&key)?;
                Ok((true, val))
            }
        }
    }

    pub fn destroy_if_empty(&mut self, owner: &ContractAddress, id: usize) -> VmrtRes<bool> {
        if self.len(owner, id)? > 0 {
            return Ok(false);
        }
        self.destroy(owner, id)?;
        Ok(true)
    }

    pub fn keys_page(
        &self,
        owner: &ContractAddress,
        id: usize,
        cursor: usize,
        limit: usize,
    ) -> VmrtRes<(Option<usize>, Vec<Vec<u8>>)> {
        if limit == 0 {
            return itr_err_fmt!(ItrErrCode::IntentError, "intent keys page limit must be positive");
        }
        let keys = self.keys_sorted(owner, id)?;
        if keys.is_empty() {
            if cursor == 0 {
                return Ok((None, vec![]));
            }
            return itr_err_fmt!(ItrErrCode::IntentError, "intent keys page cursor out of range");
        }
        if cursor > keys.len() {
            return itr_err_fmt!(ItrErrCode::IntentError, "intent keys page cursor out of range");
        }
        if cursor == keys.len() {
            return Ok((None, vec![]));
        }
        let end = cursor.saturating_add(limit).min(keys.len());
        let next = if end < keys.len() { Some(end) } else { None };
        Ok((next, keys[cursor..end].to_vec()))
    }

    pub fn move_key(
        &mut self,
        owner: &ContractAddress,
        id: usize,
        src_key: Value,
        dst_key: Value,
    ) -> VmrtErr {
        self.validate_intent_key(&src_key)?;
        let val = {
            let entry = self.require_ref(owner, id)?;
            if !entry.data.contains_key(&src_key)? {
                return itr_err_fmt!(ItrErrCode::IntentError, "intent source key not found");
            }
            self.validate_intent_key(&dst_key)?;
            if entry.data.contains_key(&dst_key)? {
                return itr_err_fmt!(ItrErrCode::IntentError, "intent destination key already exists");
            }
            entry.data.get(&src_key)?
        };
        self.validate_key_value_for_put(&dst_key, &val)?;
        self.require_mut(owner, id)?.data.remove(&src_key)?;
        self.put(owner, id, dst_key, val)?;
        Ok(())
    }

    pub fn keys_from(
        &self,
        owner: &ContractAddress,
        id: usize,
        start: Option<&Value>,
        limit: usize,
    ) -> VmrtRes<(Option<Vec<u8>>, Vec<Vec<u8>>)> {
        if limit == 0 {
            return itr_err_fmt!(ItrErrCode::IntentError, "intent keys from limit must be positive");
        }
        let keys = self.keys_sorted(owner, id)?;
        if keys.is_empty() {
            return Ok((None, vec![]));
        }
        let from = match start {
            None => 0usize,
            Some(key) => {
                let key = self.extract_intent_key_bytes(key)?;
                match keys.binary_search(&key) {
                    Ok(i) => i + 1,
                    Err(i) => i,
                }
            }
        };
        let end = from.saturating_add(limit).min(keys.len());
        let page = keys[from..end].to_vec();
        let next = if end < keys.len() {
            Some(page[page.len() - 1].clone())
        } else {
            None
        };
        Ok((next, page))
    }

    pub fn put_many(
        &mut self,
        owner: &ContractAddress,
        id: usize,
        pairs: Vec<(Value, Value)>,
    ) -> VmrtErr {
        let mut uniq = HashSet::new();
        for (key, val) in &pairs {
            self.validate_key_value_for_put(key, val)?;
            let key_bytes = self.extract_intent_key_bytes(key)?;
            if !uniq.insert(key_bytes) {
                return itr_err_fmt!(ItrErrCode::IntentError, "intent put_pairs duplicate key in batch");
            }
        }
        let entry = self.require_ref(owner, id)?;
        let mut add = 0usize;
        for (key, _) in &pairs {
            if !entry.data.contains_key(key)? {
                add = add
                    .checked_add(1)
                    .ok_or_else(|| ItrErr::new(ItrErrCode::IntentError, "intent key count overflow"))?;
            }
        }
        let total = entry
            .data
            .len()
            .checked_add(add)
            .ok_or_else(|| ItrErr::new(ItrErrCode::IntentError, "intent key count overflow"))?;
        if total > self.intent_key_limit {
            return itr_err_fmt!(
                ItrErrCode::IntentError,
                "intent key count {} exceeds limit {}",
                total,
                self.intent_key_limit
            );
        }
        let entry = self.require_mut(owner, id)?;
        for (key, val) in pairs {
            entry.data.put(key, val)?;
        }
        Ok(())
    }

    pub fn put_if_absent_or_match(
        &mut self,
        owner: &ContractAddress,
        id: usize,
        key: Value,
        val: Value,
    ) -> VmrtRes<bool> {
        self.validate_key_value_for_put(&key, &val)?;
        let intent_key_limit = self.intent_key_limit;
        let entry = self.require_mut(owner, id)?;
        if entry.data.contains_key(&key)? {
            let existing = entry.data.get(&key)?;
            if existing == val {
                return Ok(false);
            }
            return itr_err_fmt!(ItrErrCode::IntentError, "intent existing value mismatch");
        }
        Self::ensure_insert_capacity(entry, &key, intent_key_limit)?;
        entry.data.put(key, val)?;
        Ok(true)
    }

    pub fn has_all(
        &self,
        owner: &ContractAddress,
        id: usize,
        keys: &[Value],
    ) -> VmrtRes<bool> {
        self.ensure_unique_batch_keys(keys, "has_all")?;
        let entry = self.require_ref(owner, id)?;
        for key in keys {
            if !entry.data.contains_key(key)? {
                return Ok(false);
            }
        }
        Ok(true)
    }

    pub fn has_any(
        &self,
        owner: &ContractAddress,
        id: usize,
        keys: &[Value],
    ) -> VmrtRes<bool> {
        self.ensure_unique_batch_keys(keys, "has_any")?;
        let entry = self.require_ref(owner, id)?;
        for key in keys {
            if entry.data.contains_key(key)? {
                return Ok(true);
            }
        }
        Ok(false)
    }

    // Core batch read: returns (key_bytes, value) pairs
    fn batch_read_core(
        &self,
        owner: &ContractAddress,
        id: usize,
        keys: &[Value],
        op: &str,
    ) -> VmrtRes<Vec<(Vec<u8>, Value)>> {
        self.ensure_unique_batch_keys(keys, op)?;
        let entry = self.require_ref(owner, id)?;
        let mut pairs = Vec::with_capacity(keys.len());
        for key in keys {
            let key_bytes = self.extract_intent_key_bytes(key)?;
            let val = entry.data.get(key)?;
            if val.is_nil() {
                return Err(Self::key_not_found());
            }
            pairs.push((key_bytes, val));
        }
        Ok(pairs)
    }

    // Core batch remove: validates all keys exist, then removes them
    fn batch_remove_core(
        &mut self,
        owner: &ContractAddress,
        id: usize,
        keys: &[Value],
        op: &str,
    ) -> VmrtErr {
        self.ensure_unique_batch_keys(keys, op)?;
        // Validate all keys exist first
        {
            let entry = self.require_ref(owner, id)?;
            for key in keys {
                if !entry.data.contains_key(key)? {
                    return Err(Self::key_not_found());
                }
            }
        }
        // Then remove all
        let entry = self.require_mut(owner, id)?;
        for key in keys {
            entry.data.remove(key)?;
        }
        Ok(())
    }

    pub fn require_many(
        &self,
        owner: &ContractAddress,
        id: usize,
        keys: &[Value],
    ) -> VmrtRes<Vec<Value>> {
        let pairs = self.batch_read_core(owner, id, keys, "require_many")?;
        Ok(pairs.into_iter().map(|(_, v)| v).collect())
    }

    pub fn require_map(
        &self,
        owner: &ContractAddress,
        id: usize,
        keys: &[Value],
    ) -> VmrtRes<BTreeMap<Vec<u8>, Value>> {
        let pairs = self.batch_read_core(owner, id, keys, "require_map")?;
        Ok(BTreeMap::from_iter(pairs))
    }

    pub fn del_many(
        &mut self,
        owner: &ContractAddress,
        id: usize,
        keys: &[Value],
    ) -> VmrtRes<usize> {
        self.batch_remove_core(owner, id, keys, "del_many")?;
        Ok(keys.len())
    }

    pub fn take_many(
        &mut self,
        owner: &ContractAddress,
        id: usize,
        keys: &[Value],
    ) -> VmrtRes<Vec<Value>> {
        let pairs = self.batch_read_core(owner, id, keys, "take_many")?;
        self.batch_remove_core(owner, id, keys, "take_many")?;
        Ok(pairs.into_iter().map(|(_, v)| v).collect())
    }

    pub fn take_map(
        &mut self,
        owner: &ContractAddress,
        id: usize,
        keys: &[Value],
    ) -> VmrtRes<BTreeMap<Vec<u8>, Value>> {
        let pairs = self.batch_read_core(owner, id, keys, "take_map")?;
        self.batch_remove_core(owner, id, keys, "take_map")?;
        Ok(BTreeMap::from_iter(pairs))
    }

    // Helper: destroy intent if it's now empty
    fn destroy_if_now_empty(&mut self, owner: &ContractAddress, id: usize) -> VmrtErr {
        if self.len(owner, id)? == 0 {
            self.destroy(owner, id)?;
        }
        Ok(())
    }

    pub fn consume(&mut self, owner: &ContractAddress, id: usize, key: &Value) -> VmrtRes<Value> {
        let val = self.take(owner, id, key)?;
        self.destroy_if_now_empty(owner, id)?;
        Ok(val)
    }

    pub fn consume_many(
        &mut self,
        owner: &ContractAddress,
        id: usize,
        keys: &[Value],
    ) -> VmrtRes<Vec<Value>> {
        let vals = self.take_many(owner, id, keys)?;
        self.destroy_if_now_empty(owner, id)?;
        Ok(vals)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct DeferredEntry {
    pub addr: ContractAddress,
    pub intent_id: Option<usize>,
}

#[derive(Clone, Debug)]
pub struct DeferredRegistry {
    active: bool,
    entries: Vec<DeferredEntry>,
    seen: HashSet<DeferredEntry>,
}

impl Default for DeferredRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl DeferredRegistry {
    pub fn new() -> Self {
        Self {
            active: true,
            entries: Vec::new(),
            seen: HashSet::new(),
        }
    }

    pub fn clear(&mut self) {
        self.active = true;
        self.entries.clear();
        self.seen.clear();
    }

    pub fn register(&mut self, entry: DeferredEntry) -> VmrtErr {
        if !self.active {
            return itr_err_fmt!(
                ItrErrCode::DeferredError,
                "defer is closed during deferred dispatch"
            );
        }
        if self.seen.insert(entry.clone()) {
            self.entries.push(entry);
        }
        Ok(())
    }

    pub fn drain_lifo(&mut self) -> Vec<DeferredEntry> {
        self.active = false;
        self.entries.drain(..).rev().collect()
    }
}

pub type DeferCallbacks = DeferredRegistry;

#[derive(Default)]
pub struct Resoure {
    cfg_height: u64,   // height used to build current config
    next_upgrade: u64, // cached: next upgrade height (skip rebuild if height < this)
    pub gas_table: GasTable,
    pub gas_extra: GasExtra,
    pub space_cap: SpaceCap,
    pub global_map: GKVMap,
    pub memory_map: CtcKVMap,
    pub intents: IntentRuntime,
    pub contracts: HashMap<ContractAddress, Arc<ContractObj>>,
    pub gas_use: GasUse,
    pub stack_pool: Vec<Stack>,
    pub heap_pool: Vec<Heap>,
    pub deferred_registry: DeferredRegistry,
}

impl Resoure {
    pub fn create(height: u64) -> Self {
        let cap = SpaceCap::new(height);
        Self {
            cfg_height: height,
            next_upgrade: Self::next_upgrade_after(height),
            global_map: GKVMap::new(cap.global),
            memory_map: CtcKVMap::new(cap.memory),
            intents: IntentRuntime::new(cap.intent_new, cap.intent_key, cap.value_size, cap.intent_key),
            space_cap: cap,
            gas_extra: GasExtra::new(height),
            gas_table: GasTable::new(height),
            deferred_registry: DeferredRegistry::new(),
            ..Default::default()
        }
    }

    pub fn reclaim(&mut self) {
        self.gas_use = GasUse::default();
        self.global_map.clear();
        self.memory_map.clear();
        self.intents.clear();
        self.contracts.clear();
        self.deferred_registry.clear();
    }

    pub fn reset(&mut self, height: u64) {
        // Rebuild config when height rolls back below current cfg_height, or crosses next upgrade.
        if height >= self.cfg_height && height < self.next_upgrade {
            return; // same protocol version, skip config rebuild
        }
        // crossed an upgrade boundary — rebuild config
        self.reset_gascap(height);
    }

    fn reset_gascap(&mut self, height: u64) {
        let cap = SpaceCap::new(height);
        self.cfg_height = height;
        self.next_upgrade = Self::next_upgrade_after(height);
        // rebuild
        self.global_map.reset(cap.global);
        self.memory_map.reset(cap.memory);
        self.intents.reset(cap.intent_new, cap.intent_key, cap.value_size, cap.intent_key);
        self.space_cap = cap;
        self.gas_extra = GasExtra::new(height);
        self.gas_table = GasTable::new(height);
    }

    #[inline(always)]
    pub fn reset_call_gas_use(&mut self) {
        self.gas_use = GasUse::default();
    }

    #[inline(always)]
    pub fn gas_use(&self) -> GasUse {
        self.gas_use
    }

    #[inline(always)]
    pub fn next_compute_used(&self, add: i64) -> VmrtRes<i64> {
        if add < 0 {
            return itr_err_fmt!(ItrErrCode::GasError, "gas cost invalid: {}", add);
        }
        let next = self
            .gas_use
            .compute
            .checked_add(add)
            .ok_or_else(|| ItrErr::new(ItrErrCode::OutOfGas, "compute gas overflow"))?;
        let limit = self.gas_extra.compute_limit;
        if limit > 0 && next > limit {
            return itr_err_fmt!(
                ItrErrCode::OutOfGas,
                "compute gas limit exceeded: used {} > limit {}",
                next,
                limit
            );
        }
        Ok(next)
    }

    #[inline(always)]
    pub fn next_resource_used(&self, add: i64) -> VmrtRes<i64> {
        if add < 0 {
            return itr_err_fmt!(ItrErrCode::GasError, "gas cost invalid: {}", add);
        }
        let next = self
            .gas_use
            .resource
            .checked_add(add)
            .ok_or_else(|| ItrErr::new(ItrErrCode::OutOfGas, "resource gas overflow"))?;
        let limit = self.gas_extra.resource_limit;
        if limit > 0 && next > limit {
            return itr_err_fmt!(
                ItrErrCode::OutOfGas,
                "resource gas limit exceeded: used {} > limit {}",
                next,
                limit
            );
        }
        Ok(next)
    }

    #[cfg(feature = "calcfunc")]
    #[inline(always)]
    pub fn calc_resource_gas_limit<H: VmHost + ?Sized>(&self, host: &H) -> VmrtRes<i64> {
        let host_limit = host.gas_remaining();
        if host_limit < 0 {
            return itr_err_code!(ItrErrCode::OutOfGas);
        }
        let bucket_limit = self.gas_extra.resource_limit;
        if bucket_limit <= 0 {
            return Ok(host_limit);
        }
        if self.gas_use.resource > bucket_limit {
            return itr_err_fmt!(
                ItrErrCode::OutOfGas,
                "resource gas limit exceeded: used {} > limit {}",
                self.gas_use.resource,
                bucket_limit
            );
        }
        Ok(host_limit.min(bucket_limit - self.gas_use.resource))
    }

    #[inline(always)]
    pub fn next_storage_used(&self, add: i64) -> VmrtRes<i64> {
        if add < 0 {
            return itr_err_fmt!(ItrErrCode::GasError, "gas cost invalid: {}", add);
        }
        let next = self
            .gas_use
            .storage
            .checked_add(add)
            .ok_or_else(|| ItrErr::new(ItrErrCode::OutOfGas, "storage gas overflow"))?;
        let limit = self.gas_extra.storage_limit;
        if limit > 0 && next > limit {
            return itr_err_fmt!(
                ItrErrCode::OutOfGas,
                "storage gas limit exceeded: used {} > limit {}",
                next,
                limit
            );
        }
        Ok(next)
    }

    #[inline(always)]
    pub fn commit_gas_use(&mut self, compute: i64, resource: i64, storage: i64) {
        self.gas_use.compute = compute;
        self.gas_use.resource = resource;
        self.gas_use.storage = storage;
    }

    // Charge one cold contract load with per-load bytes fee.
    pub fn settle_new_contract_load_gas<H: VmHost + ?Sized>(
        &mut self,
        host: &mut H,
        bytes: usize,
    ) -> VmrtErr {
        let gas = self.gas_extra.new_contract_load +
            self.gas_extra.contract_bytes(bytes);
        let next_resource = self.next_resource_used(gas)?;
        host.gas_charge(gas)?;
        self.gas_use.resource = next_resource;
        Ok(())
    }

    #[cfg(feature = "calcfunc")]
    pub fn settle_calc_resource_gas<H: VmHost + ?Sized>(
        &mut self,
        host: &mut H,
        gas: i64,
    ) -> VmrtErr {
        let next_resource = self.next_resource_used(gas)?;
        host.gas_charge(gas)?;
        self.gas_use.resource = next_resource;
        Ok(())
    }

    pub fn stack_allocat(&mut self) -> Stack {
        self.stack_pool.pop().unwrap_or(Stack::default())
    }

    pub fn stack_reclaim(&mut self, stk: Stack) {
        self.stack_pool.push(stk);
    }

    pub fn heap_allocat(&mut self) -> Heap {
        self.heap_pool.pop().unwrap_or(Heap::default())
    }

    pub fn heap_reclaim(&mut self, heap: Heap) {
        self.heap_pool.push(heap);
    }

    // util

    /// Return the smallest upgrade height that is strictly greater than `height`.
    /// If no future upgrade exists, returns `u64::MAX`.
    fn next_upgrade_after(height: u64) -> u64 {
        for &h in UPGRADE_HEIGHTS {
            if h > height {
                return h;
            }
        }
        u64::MAX
    }
}

#[cfg(test)]
mod resource_tests {
    use super::*;
    use sys::XRet;

    struct GasHost {
        remaining: i64,
    }

    impl VmHost for GasHost {
        fn height(&self) -> u64 {
            1
        }

        fn main_entry_bindings(&self) -> FrameBindings {
            FrameBindings::root(Address::default(), Vec::<Address>::new().into())
        }

        fn gas_remaining(&self) -> i64 {
            self.remaining
        }

        fn gas_charge(&mut self, gas: i64) -> VmrtErr {
            if gas < 0 {
                return itr_err_fmt!(GasError, "gas cost invalid: {}", gas);
            }
            self.remaining -= gas;
            if self.remaining < 0 {
                return itr_err_code!(OutOfGas);
            }
            Ok(())
        }

        fn gas_rebate(&mut self, gas: i64) -> VmrtErr {
            let _ = gas;
            Ok(())
        }

        fn contract_edition(&mut self, _: &ContractAddress) -> Option<ContractEdition> {
            None
        }

        fn contract(&mut self, _: &ContractAddress) -> Option<ContractSto> {
            None
        }

        fn action_call(&mut self, _: u16, _: Vec<u8>) -> XRet<(u32, Vec<u8>)> {
            unreachable!()
        }

        fn log_push(&mut self, _: &Address, _: Vec<Value>) -> VmrtErr {
            unreachable!()
        }

        fn sstat(&mut self, _: &GasExtra, _: &SpaceCap, _: &Address, _: &Value) -> VmrtRes<Value> {
            unreachable!()
        }

        fn sload(&mut self, _: &GasExtra, _: &SpaceCap, _: &Address, _: &Value) -> VmrtRes<Value> {
            unreachable!()
        }

        fn sdel(&mut self, _: &GasExtra, _: &SpaceCap, _: &Address, _: Value) -> VmrtRes<i64> {
            unreachable!()
        }

        fn snew(
            &mut self,
            _: &GasExtra,
            _: &SpaceCap,
            _: &Address,
            _: Value,
            _: Value,
            _: Value,
        ) -> VmrtRes<i64> {
            unreachable!()
        }

        fn sedit(&mut self, _: &GasExtra, _: &SpaceCap, _: &Address, _: Value, _: Value) -> VmrtRes<i64> {
            unreachable!()
        }

        fn srent(&mut self, _: &GasExtra, _: &SpaceCap, _: &Address, _: Value, _: Value) -> VmrtRes<i64> {
            unreachable!()
        }

        fn srecv(&mut self, _: &GasExtra, _: &SpaceCap, _: &Address, _: Value, _: Value) -> VmrtRes<i64> {
            unreachable!()
        }
    }

    #[test]
    fn settle_new_contract_load_gas_charges_base_plus_bytes_div_50() {
        let mut r = Resoure::create(1);
        let base = r.gas_extra.new_contract_load;
        let mut host = GasHost { remaining: 1000 };
        r.settle_new_contract_load_gas(&mut host, 129).unwrap();
        assert_eq!(host.remaining, 1000 - base - 3);
    }

    #[test]
    fn reset_rebuilds_config_when_height_rolls_back() {
        let mut r = Resoure::create(200);
        assert_eq!(r.cfg_height, 200);
        r.reset(100);
        assert_eq!(r.cfg_height, 100);
    }

    #[test]
    fn intent_runtime_enforces_create_limit() {
        let owner = ContractAddress::from_unchecked(Address::create_contract([9u8; 20]));
        let mut rt = IntentRuntime::new(200, 16, 1280, 128);
        for _ in 0..200 {
            rt.create(owner.clone(), b"x".to_vec()).unwrap();
        }
        let err = rt.create(owner, b"x".to_vec()).unwrap_err();
        assert_eq!(err.0, ItrErrCode::IntentError);
        assert!(err.1.contains("intent creation limit"));
    }

    #[test]
    fn defer_callbacks_are_deduped_and_drained_lifo() {
        let mut callbacks = DeferCallbacks::new();
        let a = ContractAddress::from_unchecked(Address::create_contract([1u8; 20]));
        let b = ContractAddress::from_unchecked(Address::create_contract([2u8; 20]));
        callbacks
            .register(DeferredEntry {
                addr: a.clone(),
                intent_id: None,
            })
            .unwrap();
        callbacks
            .register(DeferredEntry {
                addr: a.clone(),
                intent_id: None,
            })
            .unwrap();
        callbacks
            .register(DeferredEntry {
                addr: b.clone(),
                intent_id: Some(7),
            })
            .unwrap();
        let drained = callbacks.drain_lifo();
        assert_eq!(
            drained,
            vec![
                DeferredEntry {
                    addr: b,
                    intent_id: Some(7)
                },
                DeferredEntry {
                    addr: a,
                    intent_id: None
                },
            ]
        );
    }

    #[test]
    fn intent_keys_from_pagination_returns_correct_next_key() {
        let owner = ContractAddress::from_unchecked(Address::create_contract([1u8; 20]));
        let mut rt = IntentRuntime::new(100, 16, 1280, 128);
        let id = rt.create(owner.clone(), b"test".to_vec()).unwrap();

        // Insert keys: a, b, c, d, e
        for key in [b"a", b"b", b"c", b"d", b"e"] {
            rt.put(&owner, id, Value::Bytes(key.to_vec()), Value::U64(1)).unwrap();
        }

        // First page: get 2 keys starting from None
        let (next, page) = rt.keys_from(&owner, id, None, 2).unwrap();
        assert_eq!(page, vec![b"a".to_vec(), b"b".to_vec()]);
        assert_eq!(next, Some(b"b".to_vec())); // Resume token must be reusable as next start

        // Second page: use next as start
        let next_val = Value::Bytes(next.unwrap());
        let (next2, page2) = rt.keys_from(&owner, id, Some(&next_val), 2).unwrap();
        assert_eq!(page2, vec![b"c".to_vec(), b"d".to_vec()]);
        assert_eq!(next2, Some(b"d".to_vec()));

        // Third page: consume the tail
        let next_val2 = Value::Bytes(next2.unwrap());
        let (next3, page3) = rt.keys_from(&owner, id, Some(&next_val2), 2).unwrap();
        assert_eq!(page3, vec![b"e".to_vec()]);
        assert_eq!(next3, None);
    }
}
