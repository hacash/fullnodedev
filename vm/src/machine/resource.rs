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

#[derive(Clone, Debug)]
pub struct IntentRuntime {
    next_id: u64,
    intent_limit: usize,
    key_limit: usize,
    intents: HashMap<u64, IntentEntry>,
}

impl Default for IntentRuntime {
    fn default() -> Self {
        Self::new(0, 0)
    }
}

impl IntentRuntime {
    pub fn new(intent_limit: usize, key_limit: usize) -> Self {
        Self {
            next_id: 0,
            intent_limit,
            key_limit,
            intents: HashMap::new(),
        }
    }

    pub fn clear(&mut self) {
        self.next_id = 0;
        self.intents.clear();
    }

    pub fn reset(&mut self, intent_limit: usize, key_limit: usize) {
        self.intent_limit = intent_limit;
        self.key_limit = key_limit;
        self.clear();
    }

    pub fn create(&mut self, kind: Vec<u8>) -> VmrtRes<u64> {
        if kind.is_empty() {
            return itr_err_fmt!(ItrErrCode::IntentError, "intent kind cannot be empty");
        }
        if self.intents.len() >= self.intent_limit {
            return itr_err_fmt!(ItrErrCode::IntentError, "intent limit exceeded");
        }
        let next = self
            .next_id
            .checked_add(1)
            .ok_or_else(|| ItrErr::new(ItrErrCode::IntentError, "intent id overflow"))?;
        self.next_id = next;
        self.intents.insert(
            next,
            IntentEntry {
                kind,
                data: MKVMap::new(self.key_limit),
            },
        );
        Ok(next)
    }

    fn require_mut(&mut self, id: u64) -> VmrtRes<&mut IntentEntry> {
        self.intents
            .get_mut(&id)
            .ok_or_else(|| ItrErr::new(ItrErrCode::IntentError, &format!("intent {} not found", id)))
    }

    fn require_ref(&self, id: u64) -> VmrtRes<&IntentEntry> {
        self.intents
            .get(&id)
            .ok_or_else(|| ItrErr::new(ItrErrCode::IntentError, &format!("intent {} not found", id)))
    }

    pub fn put(&mut self, id: u64, key: Value, val: Value) -> VmrtErr {
        self.require_mut(id)?.data.put(key, val)
    }

    pub fn exists(&self, id: u64) -> bool {
        self.intents.contains_key(&id)
    }

    pub fn get(&self, id: u64, key: &Value) -> VmrtRes<Value> {
        self.require_ref(id)?.data.get(key)
    }

    pub fn take(&mut self, id: u64, key: &Value) -> VmrtRes<Value> {
        let val = self.get(id, key)?;
        if !matches!(val, Value::Nil) {
            self.require_mut(id)?.data.remove(key)?;
        }
        Ok(val)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct DeferredEntry {
    pub addr: ContractAddress,
    pub intent_id: Option<u64>,
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
            intents: IntentRuntime::new(cap.global, cap.memory),
            space_cap: cap,
            gas_extra: GasExtra::new(height),
            gas_table: GasTable::new(height),
            deferred_registry: DeferredRegistry::new(),
            ..Default::default()
        }
    }

    pub fn reclaim(&mut self) {
        self.global_map.clear();
        self.memory_map.clear();
        self.intents.clear();
        self.contracts.clear();
        self.deferred_registry.clear();
    }

    pub fn reset(&mut self, height: u64) {
        self.gas_use = GasUse::default();
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
        self.intents.reset(cap.global, cap.memory);
        self.space_cap = cap;
        self.gas_extra = GasExtra::new(height);
        self.gas_table = GasTable::new(height);
        self.gas_use = GasUse::default();
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
        let gas = self.gas_extra.new_contract_load + (bytes as i64 / 64);
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

        fn sinfo(&mut self, _: &Address, _: &Value) -> VmrtRes<Value> {
            unreachable!()
        }

        fn sload(&mut self, _: &Address, _: &Value) -> VmrtRes<Value> {
            unreachable!()
        }

        fn sdel(&mut self, _: &Address, _: Value) -> VmrtRes<i64> {
            unreachable!()
        }

        fn snew(
            &mut self,
            _: &GasExtra,
            _: &Address,
            _: Value,
            _: Value,
            _: Value,
        ) -> VmrtRes<i64> {
            unreachable!()
        }

        fn sedit(&mut self, _: &GasExtra, _: &Address, _: Value, _: Value) -> VmrtRes<i64> {
            unreachable!()
        }

        fn srent(&mut self, _: &GasExtra, _: &Address, _: Value, _: Value) -> VmrtRes<i64> {
            unreachable!()
        }

        fn srecv(&mut self, _: &GasExtra, _: &Address, _: Value, _: Value) -> VmrtRes<i64> {
            unreachable!()
        }
    }

    #[test]
    fn settle_new_contract_load_gas_charges_base_plus_bytes_div_64() {
        let mut r = Resoure::create(1);
        let base = r.gas_extra.new_contract_load;
        let mut host = GasHost { remaining: 1000 };
        r.settle_new_contract_load_gas(&mut host, 129).unwrap();
        assert_eq!(host.remaining, 1000 - base - 2);
    }

    #[test]
    fn reset_rebuilds_config_when_height_rolls_back() {
        let mut r = Resoure::create(200);
        assert_eq!(r.cfg_height, 200);
        r.reset(100);
        assert_eq!(r.cfg_height, 100);
    }

    #[test]
    fn defer_callbacks_are_deduped_and_drained_lifo() {
        let mut callbacks = DeferCallbacks::new();
        let a = ContractAddress::from_unchecked(Address::create_contract([1u8; 20]));
        let b = ContractAddress::from_unchecked(Address::create_contract([2u8; 20]));
        callbacks
            .register(DeferredEntry { addr: a.clone(), intent_id: None })
            .unwrap();
        callbacks
            .register(DeferredEntry { addr: a.clone(), intent_id: None })
            .unwrap();
        callbacks
            .register(DeferredEntry { addr: b.clone(), intent_id: Some(7) })
            .unwrap();
        let drained = callbacks.drain_lifo();
        assert_eq!(
            drained,
            vec![
                DeferredEntry { addr: b, intent_id: Some(7) },
                DeferredEntry { addr: a, intent_id: None },
            ]
        );
    }
}
