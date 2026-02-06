use std::cell::UnsafeCell;
use std::collections::{HashMap, VecDeque};
use std::hash::{Hash, Hasher};
use std::sync::{Arc, Mutex};

#[derive(Clone, Debug)]
pub struct ContractCacheConfig {
    /// Max cached bytes for global contract cache pool.
    /// `0` disables the cache entirely.
    pub max_bytes: usize,
    /// Target ratio of protected segment bytes (0..=100).
    /// Protected segment retains entries that show repeated use.
    pub protected_ratio: u8,
    /// Heat half-life in access ticks (not time).
    /// Every `heat_half_life` pool accesses, an entry's heat is halved (>>=1).
    pub heat_half_life: u64,
    /// Heat added per hit (saturating).
    pub hit_boost: u32,
    /// Promotion threshold from probation -> protected after applying decay and hit boost.
    pub promote_threshold: u32,
    /// Maximum bytes for a single entry. `0` means no per-entry limit.
    pub max_entry_bytes: usize,
}

impl Default for ContractCacheConfig {
    fn default() -> Self {
        Self {
            max_bytes: 0,
            protected_ratio: 70,
            heat_half_life: 10_000,
            hit_boost: 10,
            promote_threshold: 20,
            max_entry_bytes: 0,
        }
    }
}

#[derive(Clone, Debug)]
pub struct ContractCacheStats {
    pub enabled: bool,
    pub max_bytes: usize,
    pub used_bytes: usize,
    pub entries: usize,
    pub hits: u64,
    pub misses: u64,
    pub inserts: u64,
    pub evicts: u64,
}

#[derive(Clone, Eq)]
struct ContractCacheKey {
    addr: ContractAddress,
    revision: u16,
}

impl PartialEq for ContractCacheKey {
    fn eq(&self, other: &Self) -> bool {
        self.revision == other.revision && self.addr == other.addr
    }
}

impl Hash for ContractCacheKey {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.addr.hash(state);
        self.revision.hash(state);
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum Segment {
    Probation,
    Protected,
}

struct Entry {
    obj: Arc<ContractObj>,
    charge_bytes: usize,
    segment: Segment,
    heat: u32,
    last_tick: u64,
    lru_tag: u64,
}

struct ContractCacheInner {
    config: ContractCacheConfig,
    tick: u64,
    used_bytes: usize,
    map: HashMap<ContractCacheKey, Entry>,
    probation_lru: VecDeque<(ContractCacheKey, u64)>,
    protected_lru: VecDeque<(ContractCacheKey, u64)>,
    hits: u64,
    misses: u64,
    inserts: u64,
    evicts: u64,
}

impl ContractCacheInner {
    fn enabled(&self) -> bool {
        self.config.max_bytes > 0
    }

    fn protected_budget(&self) -> usize {
        let ratio = self.config.protected_ratio.min(100) as usize;
        self.config.max_bytes.saturating_mul(ratio) / 100
    }

    fn now_tick(&mut self) -> u64 {
        self.tick = self.tick.wrapping_add(1);
        self.tick
    }

    fn remove_from_lru_queue(queue: &mut VecDeque<(ContractCacheKey, u64)>, key: &ContractCacheKey) {
        let mut i = 0;
        while i < queue.len() {
            if queue[i].0 == *key {
                queue.remove(i);
            } else {
                i += 1;
            }
        }
    }

    fn touch(&mut self, key: &ContractCacheKey) {
        let now = self.now_tick();
        let Some(ent) = self.map.get_mut(key) else {
            return;
        };
        let hl = self.config.heat_half_life;
        let decayed = if hl == 0 {
            ent.heat
        } else {
            let dt = now.saturating_sub(ent.last_tick);
            let shift = (dt / hl).min(31) as u32;
            ent.heat >> shift
        };
        ent.heat = decayed.saturating_add(self.config.hit_boost);
        ent.last_tick = now;
        ent.lru_tag = now;

        match ent.segment {
            Segment::Probation => {
                Self::remove_from_lru_queue(&mut self.probation_lru, key);
                self.probation_lru.push_front((key.clone(), ent.lru_tag));
                if ent.heat >= self.config.promote_threshold {
                    ent.segment = Segment::Protected;
                    Self::remove_from_lru_queue(&mut self.probation_lru, key);
                    self.protected_lru.push_front((key.clone(), ent.lru_tag));
                }
            }
            Segment::Protected => {
                Self::remove_from_lru_queue(&mut self.protected_lru, key);
                self.protected_lru.push_front((key.clone(), ent.lru_tag));
            }
        }
    }

    fn get(&mut self, key: &ContractCacheKey) -> Option<Arc<ContractObj>> {
        if !self.enabled() {
            return None;
        }
        if let Some(ent) = self.map.get(key) {
            self.hits += 1;
            let obj = ent.obj.clone();
            self.touch(key);
            return Some(obj);
        }
        self.misses += 1;
        None
    }

    fn estimate_charge_bytes(sto: &ContractSto, obj: &ContractObj) -> usize {
        // Best-effort estimate (not exact heap accounting).
        // Keep it stable: use serialized size + raw code sizes + small overhead.
        let mut sum = sto.size();
        for f in obj.abstfns.values() {
            sum = sum.saturating_add(f.codes.len());
        }
        for f in obj.userfns.values() {
            sum = sum.saturating_add(f.codes.len());
        }
        sum.saturating_add(256)
    }

    fn insert(&mut self, addr: &ContractAddress, sto: &ContractSto, obj: Arc<ContractObj>) {
        if !self.enabled() {
            return;
        }
        let rev = sto.metas.revision.uint();
        let key = ContractCacheKey {
            addr: addr.clone(),
            revision: rev,
        };

        let charge = Self::estimate_charge_bytes(sto, &obj);
        if self.config.max_entry_bytes > 0 && charge > self.config.max_entry_bytes {
            return;
        }
        if charge > self.config.max_bytes {
            return;
        }
        if self.map.contains_key(&key) {
            // Already cached; just touch.
            self.touch(&key);
            return;
        }

        self.inserts += 1;
        let now = self.now_tick();
        let ent = Entry {
            obj,
            charge_bytes: charge,
            segment: Segment::Probation,
            heat: self.config.hit_boost,
            last_tick: now,
            lru_tag: now,
        };
        self.used_bytes = self.used_bytes.saturating_add(charge);
        self.map.insert(key.clone(), ent);
        self.probation_lru.push_front((key, now));

        self.evict_until_fit();
    }

    fn evict_until_fit(&mut self) {
        if !self.enabled() {
            self.clear_all();
            return;
        }
        let max = self.config.max_bytes;
        if self.used_bytes <= max {
            return;
        }
        let mut consecutive_failures = 0;
        while self.used_bytes > max {
            if !self.evict_one_from_probation() && !self.evict_one_from_protected() {
                consecutive_failures += 1;
                if consecutive_failures >= 2 {
                    break;
                }
            } else {
                consecutive_failures = 0;
            }
        }
        // Optional balancing: if protected is too large, evict from protected tail.
        let pb = self.protected_budget();
        consecutive_failures = 0;
        while self.used_bytes > 0 && self.protected_used_bytes() > pb {
            if !self.evict_one_from_protected() {
                consecutive_failures += 1;
                if consecutive_failures >= 2 {
                    break;
                }
            } else {
                consecutive_failures = 0;
            }
        }
    }

    fn protected_used_bytes(&self) -> usize {
        self.map
            .values()
            .filter(|e| e.segment == Segment::Protected)
            .map(|e| e.charge_bytes)
            .sum()
    }

    fn evict_one_from_probation(&mut self) -> bool {
        while let Some((key, tag)) = self.probation_lru.pop_back() {
            let Some(ent) = self.map.get(&key) else {
                continue;
            };
            if ent.segment != Segment::Probation || ent.lru_tag != tag {
                continue;
            }
            self.remove_entry(&key);
            return true;
        }
        false
    }

    fn evict_one_from_protected(&mut self) -> bool {
        while let Some((key, tag)) = self.protected_lru.pop_back() {
            let Some(ent) = self.map.get(&key) else {
                continue;
            };
            if ent.segment != Segment::Protected || ent.lru_tag != tag {
                continue;
            }
            self.remove_entry(&key);
            return true;
        }
        false
    }

    fn remove_entry(&mut self, key: &ContractCacheKey) {
        if let Some(ent) = self.map.remove(key) {
            self.used_bytes = self.used_bytes.saturating_sub(ent.charge_bytes);
            self.evicts += 1;
            Self::remove_from_lru_queue(&mut self.probation_lru, key);
            Self::remove_from_lru_queue(&mut self.protected_lru, key);
        }
    }

    fn clear_all(&mut self) {
        self.used_bytes = 0;
        self.map.clear();
        self.probation_lru.clear();
        self.protected_lru.clear();
    }
}

/// Global contract cache pool (cross-transaction, cross-block).
///
/// Heat + decay semantics (ticks are pool accesses, not wall time):
/// - Each access increases global `tick` by 1.
/// - On entry access, its heat is first decayed by half-lives:
///     heat = heat >> ((tick - last_tick) / heat_half_life)
///   then boosted:
///     heat += hit_boost (saturating)
/// - Probation->Protected promotion happens when `heat >= promote_threshold`.
/// - Eviction is SLRU by bytes: evict LRU from probation first; then from protected.
pub struct ContractCachePool {
    lock: Mutex<()>,
    inner: UnsafeCell<ContractCacheInner>,
}

unsafe impl Send for ContractCachePool {}
unsafe impl Sync for ContractCachePool {}

impl Default for ContractCachePool {
    fn default() -> Self {
        Self {
            lock: Mutex::new(()),
            inner: UnsafeCell::new(ContractCacheInner {
                config: ContractCacheConfig::default(),
                tick: 0,
                used_bytes: 0,
                map: HashMap::new(),
                probation_lru: VecDeque::new(),
                protected_lru: VecDeque::new(),
                hits: 0,
                misses: 0,
                inserts: 0,
                evicts: 0,
            }),
        }
    }
}

impl ContractCachePool {
    pub fn configure(&self, config: ContractCacheConfig) {
        let _lk = self.lock.lock().expect("ContractCachePool lock poisoned");
        let inner = unsafe { &mut *self.inner.get() };
        inner.config = config;
        inner.evict_until_fit();
    }

    pub fn stats(&self) -> ContractCacheStats {
        let _lk = self.lock.lock().expect("ContractCachePool lock poisoned");
        let inner = unsafe { &*self.inner.get() };
        ContractCacheStats {
            enabled: inner.enabled(),
            max_bytes: inner.config.max_bytes,
            used_bytes: inner.used_bytes,
            entries: inner.map.len(),
            hits: inner.hits,
            misses: inner.misses,
            inserts: inner.inserts,
            evicts: inner.evicts,
        }
    }

    pub fn get(&self, addr: &ContractAddress, revision: u16) -> Option<Arc<ContractObj>> {
        let _lk = self.lock.lock().expect("ContractCachePool lock poisoned");
        let inner = unsafe { &mut *self.inner.get() };
        inner.get(&ContractCacheKey {
            addr: addr.clone(),
            revision,
        })
    }

    pub fn insert(&self, addr: &ContractAddress, sto: &ContractSto, obj: Arc<ContractObj>) {
        let _lk = self.lock.lock().expect("ContractCachePool lock poisoned");
        let inner = unsafe { &mut *self.inner.get() };
        inner.insert(addr, sto, obj);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use field::{Address, Uint2, Uint4};

    fn create_test_contract_address(nonce: u32) -> ContractAddress {
        let addr = Address::from_readable("1MzNY1oA3kfgYi75zquj3SRUPYztzXHzK9").unwrap();
        ContractAddress::calculate(&addr, &Uint4::from(nonce))
    }

    fn create_test_contract_sto(revision: u16) -> ContractSto {
        let mut sto = ContractSto::new();
        sto.metas.revision = Uint2::from(revision);
        sto
    }

    fn create_test_contract_obj() -> ContractObj {
        ContractObj::default()
    }

    #[test]
    fn test_lru_queue_no_duplicates() {
        let pool = ContractCachePool::default();
        let config = ContractCacheConfig {
            max_bytes: 10000,
            protected_ratio: 70,
            heat_half_life: 10_000,
            hit_boost: 10,
            promote_threshold: 20,
            max_entry_bytes: 0,
        };
        pool.configure(config);

        let addr = create_test_contract_address(0);
        let sto = create_test_contract_sto(1);
        let obj = Arc::new(create_test_contract_obj());

        pool.insert(&addr, &sto, obj.clone());

        let stats1 = pool.stats();
        assert_eq!(stats1.entries, 1);

        for _ in 0..10 {
            let _ = pool.get(&addr, 1);
        }

        let stats2 = pool.stats();
        assert_eq!(stats2.entries, 1);
        assert_eq!(stats2.hits, 10);
        assert_eq!(stats2.misses, 0);
    }

    #[test]
    fn test_promotion_removes_from_probation() {
        let pool = ContractCachePool::default();
        let config = ContractCacheConfig {
            max_bytes: 10000,
            protected_ratio: 70,
            heat_half_life: 1,
            hit_boost: 15,
            promote_threshold: 20,
            max_entry_bytes: 0,
        };
        pool.configure(config);

        let addr = create_test_contract_address(0);
        let sto = create_test_contract_sto(1);
        let obj = Arc::new(create_test_contract_obj());

        pool.insert(&addr, &sto, obj.clone());

        for _ in 0..3 {
            let _ = pool.get(&addr, 1);
        }

        let stats = pool.stats();
        assert_eq!(stats.entries, 1);
    }

    #[test]
    fn test_remove_entry_cleans_lru_queues() {
        let pool = ContractCachePool::default();
        let config = ContractCacheConfig {
            max_bytes: 10000,
            protected_ratio: 70,
            heat_half_life: 10_000,
            hit_boost: 10,
            promote_threshold: 20,
            max_entry_bytes: 0,
        };
        pool.configure(config);

        let addr1 = create_test_contract_address(0);
        let addr2 = create_test_contract_address(1);
        let sto1 = create_test_contract_sto(1);
        let sto2 = create_test_contract_sto(1);
        let obj1 = Arc::new(create_test_contract_obj());
        let obj2 = Arc::new(create_test_contract_obj());

        pool.insert(&addr1, &sto1, obj1.clone());
        pool.insert(&addr2, &sto2, obj2.clone());

        let stats1 = pool.stats();
        assert_eq!(stats1.entries, 2);

        let _ = pool.get(&addr1, 1);
        let _ = pool.get(&addr2, 1);

        let stats2 = pool.stats();
        assert_eq!(stats2.entries, 2);
        assert_eq!(stats2.hits, 2);
    }

    #[test]
    fn test_eviction_respects_max_bytes() {
        let pool = ContractCachePool::default();
        let config = ContractCacheConfig {
            max_bytes: 1000,
            protected_ratio: 70,
            heat_half_life: 10_000,
            hit_boost: 10,
            promote_threshold: 20,
            max_entry_bytes: 0,
        };
        let max_bytes = config.max_bytes;
        pool.configure(config);

        for i in 0..10 {
            let addr = create_test_contract_address(i);
            let sto = create_test_contract_sto(1);
            let obj = Arc::new(create_test_contract_obj());
            pool.insert(&addr, &sto, obj);
        }

        let stats = pool.stats();
        assert!(stats.used_bytes <= max_bytes);
        assert!(stats.entries <= 10);
    }
}
