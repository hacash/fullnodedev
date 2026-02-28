use std::cell::UnsafeCell;
use std::collections::{HashMap, VecDeque};
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

#[derive(Clone, Eq, PartialEq, Hash)]
struct ContractCacheKey {
    addr: ContractAddress,
    revision: u16,
    edition_hash: Hash,
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
    fn make_key(addr: &ContractAddress, edition: &ContractEdition) -> ContractCacheKey {
        ContractCacheKey {
            addr: addr.clone(),
            revision: edition.revision.uint(),
            edition_hash: edition.hash,
        }
    }

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

    fn estimate_charge_bytes(obj: &ContractObj) -> usize {
        // Best-effort estimate (not exact heap accounting). Keep it stable: use raw loaded bytes + compiled code sizes + small overhead.
        let mut sum = obj.edition.raw_size.uint() as usize;
        for f in obj.abstfns.values() {
            sum = sum.saturating_add(f.codes.len());
        }
        for f in obj.userfns.values() {
            sum = sum.saturating_add(f.codes.len());
        }
        sum.saturating_add(256)
    }

    fn insert(&mut self, addr: &ContractAddress, obj: Arc<ContractObj>) {
        if !self.enabled() {
            return;
        }
        let key = Self::make_key(addr, &obj.edition);

        let charge = Self::estimate_charge_bytes(&obj);
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

    fn remove_addr(&mut self, addr: &ContractAddress) -> usize {
        let keys: Vec<ContractCacheKey> = self
            .map
            .keys()
            .filter(|k| &k.addr == addr)
            .cloned()
            .collect();
        let removed = keys.len();
        for key in keys {
            self.remove_entry(&key);
        }
        removed
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

    pub fn get(&self, addr: &ContractAddress, edition: &ContractEdition) -> Option<Arc<ContractObj>> {
        let _lk = self.lock.lock().expect("ContractCachePool lock poisoned");
        let inner = unsafe { &mut *self.inner.get() };
        inner.get(&ContractCacheInner::make_key(addr, edition))
    }

    pub fn insert(&self, addr: &ContractAddress, obj: Arc<ContractObj>) {
        let _lk = self.lock.lock().expect("ContractCachePool lock poisoned");
        let inner = unsafe { &mut *self.inner.get() };
        inner.insert(addr, obj);
    }

    pub fn remove_addr(&self, addr: &ContractAddress) -> usize {
        let _lk = self.lock.lock().expect("ContractCachePool lock poisoned");
        let inner = unsafe { &mut *self.inner.get() };
        inner.remove_addr(addr)
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

    fn create_test_contract_edition(revision: u16, tag: u8) -> ContractEdition {
        ContractEdition {
            revision: Uint2::from(revision),
            raw_size: Uint4::from(128),
            hash: Hash::from([tag; Hash::SIZE]),
        }
    }

    fn create_test_contract_obj(edition: ContractEdition) -> ContractObj {
        let mut obj = ContractObj::default();
        obj.edition = edition;
        obj
    }

    #[test]
    fn test_remove_addr_clears_all_revisions_of_same_addr() {
        let pool = ContractCachePool::default();
        pool.configure(ContractCacheConfig {
            max_bytes: 10000,
            ..ContractCacheConfig::default()
        });
        let addr_a = create_test_contract_address(1);
        let addr_b = create_test_contract_address(2);
        let ed_a1 = create_test_contract_edition(1, 1);
        let ed_a2 = create_test_contract_edition(2, 2);
        let ed_b1 = create_test_contract_edition(1, 3);
        pool.insert(&addr_a, Arc::new(create_test_contract_obj(ed_a1)));
        pool.insert(&addr_a, Arc::new(create_test_contract_obj(ed_a2)));
        pool.insert(&addr_b, Arc::new(create_test_contract_obj(ed_b1)));
        assert!(pool.get(&addr_a, &ed_a1).is_some());
        assert!(pool.get(&addr_a, &ed_a2).is_some());
        assert!(pool.get(&addr_b, &ed_b1).is_some());
        assert_eq!(pool.remove_addr(&addr_a), 2);
        assert!(pool.get(&addr_a, &ed_a1).is_none());
        assert!(pool.get(&addr_a, &ed_a2).is_none());
        assert!(pool.get(&addr_b, &ed_b1).is_some());
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
        let ed = create_test_contract_edition(1, 1);
        let obj = Arc::new(create_test_contract_obj(ed));

        pool.insert(&addr, obj.clone());

        let stats1 = pool.stats();
        assert_eq!(stats1.entries, 1);

        for _ in 0..10 {
            let _ = pool.get(&addr, &ed);
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
        let ed = create_test_contract_edition(1, 1);
        let obj = Arc::new(create_test_contract_obj(ed));

        pool.insert(&addr, obj.clone());

        for _ in 0..3 {
            let _ = pool.get(&addr, &ed);
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
        let ed1 = create_test_contract_edition(1, 1);
        let ed2 = create_test_contract_edition(1, 2);
        let obj1 = Arc::new(create_test_contract_obj(ed1));
        let obj2 = Arc::new(create_test_contract_obj(ed2));

        pool.insert(&addr1, obj1.clone());
        pool.insert(&addr2, obj2.clone());

        let stats1 = pool.stats();
        assert_eq!(stats1.entries, 2);

        let _ = pool.get(&addr1, &ed1);
        let _ = pool.get(&addr2, &ed2);

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
            let ed = create_test_contract_edition(1, i as u8);
            let obj = Arc::new(create_test_contract_obj(ed));
            pool.insert(&addr, obj);
        }

        let stats = pool.stats();
        assert!(stats.used_bytes <= max_bytes);
        assert!(stats.entries <= 10);
    }
}
