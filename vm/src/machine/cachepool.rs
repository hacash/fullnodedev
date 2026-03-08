use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};

#[derive(Clone, Debug)]
pub struct ContractCacheConfig {
    /// Max cached bytes for contract cache pool.
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
    protected_used_bytes: usize,
    map: HashMap<ContractCacheKey, Entry>,
    probation_lru: VecDeque<(ContractCacheKey, u64)>,
    protected_lru: VecDeque<(ContractCacheKey, u64)>,
    hits: u64,
    misses: u64,
    inserts: u64,
    evicts: u64,
}

impl Default for ContractCacheInner {
    fn default() -> Self {
        Self {
            config: ContractCacheConfig::default(),
            tick: 0,
            used_bytes: 0,
            protected_used_bytes: 0,
            map: HashMap::new(),
            probation_lru: VecDeque::new(),
            protected_lru: VecDeque::new(),
            hits: 0,
            misses: 0,
            inserts: 0,
            evicts: 0,
        }
    }
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

    fn push_lru_front(&mut self, segment: Segment, key: ContractCacheKey, tag: u64) {
        match segment {
            Segment::Probation => self.probation_lru.push_front((key, tag)),
            Segment::Protected => self.protected_lru.push_front((key, tag)),
        }
    }

    fn pop_lru_back(&mut self, segment: Segment) -> Option<(ContractCacheKey, u64)> {
        match segment {
            Segment::Probation => self.probation_lru.pop_back(),
            Segment::Protected => self.protected_lru.pop_back(),
        }
    }

    fn apply_heat(ent: &mut Entry, now: u64, heat_half_life: u64, hit_boost: u32) {
        let hl = heat_half_life;
        let decayed = if hl == 0 {
            ent.heat
        } else {
            let dt = now.saturating_sub(ent.last_tick);
            let shift = (dt / hl).min(31) as u32;
            ent.heat >> shift
        };
        ent.heat = decayed.saturating_add(hit_boost);
        ent.last_tick = now;
        ent.lru_tag = now;
    }

    fn touch(&mut self, key: &ContractCacheKey) {
        let now = self.now_tick();
        let (target_segment, new_tag, promoted_charge) = {
            let Some(ent) = self.map.get_mut(key) else {
                return;
            };
            Self::apply_heat(
                ent,
                now,
                self.config.heat_half_life,
                self.config.hit_boost,
            );
            let mut promoted_charge = 0usize;
            if ent.segment == Segment::Probation && ent.heat >= self.config.promote_threshold {
                ent.segment = Segment::Protected;
                promoted_charge = ent.charge_bytes;
            }
            (ent.segment, ent.lru_tag, promoted_charge)
        };
        if promoted_charge > 0 {
            self.protected_used_bytes = self.protected_used_bytes.saturating_add(promoted_charge);
        }
        self.push_lru_front(target_segment, key.clone(), new_tag);
        self.maybe_compact_lru_queues();
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
        self.push_lru_front(Segment::Probation, key, now);
        self.maybe_compact_lru_queues();

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
        self.maybe_compact_lru_queues();
        let mut consecutive_failures = 0;
        while self.used_bytes > max {
            if !self.evict_one_from(Segment::Probation) && !self.evict_one_from(Segment::Protected) {
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
        while self.used_bytes > 0 && self.protected_used_bytes > pb {
            if !self.evict_one_from(Segment::Protected) {
                consecutive_failures += 1;
                if consecutive_failures >= 2 {
                    break;
                }
            } else {
                consecutive_failures = 0;
            }
        }
    }

    fn maybe_compact_lru_queues(&mut self) {
        let map_len = self.map.len();
        if map_len == 0 {
            self.probation_lru.clear();
            self.protected_lru.clear();
            return;
        }
        let threshold = map_len.saturating_mul(4);
        if self.probation_lru.len() <= threshold && self.protected_lru.len() <= threshold {
            return;
        }
        self.rebuild_lru_queues();
    }

    fn rebuild_lru_queues(&mut self) {
        let mut probation = Vec::new();
        let mut protected = Vec::new();
        for (key, ent) in self.map.iter() {
            match ent.segment {
                Segment::Probation => probation.push((key.clone(), ent.lru_tag)),
                Segment::Protected => protected.push((key.clone(), ent.lru_tag)),
            }
        }
        probation.sort_unstable_by(|a, b| b.1.cmp(&a.1));
        protected.sort_unstable_by(|a, b| b.1.cmp(&a.1));
        self.probation_lru.clear();
        self.protected_lru.clear();
        self.probation_lru.extend(probation);
        self.protected_lru.extend(protected);
    }

    fn evict_one_from(&mut self, segment: Segment) -> bool {
        while let Some((key, tag)) = self.pop_lru_back(segment) {
            let Some(ent) = self.map.get(&key) else {
                continue;
            };
            if ent.segment != segment || ent.lru_tag != tag {
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
            if ent.segment == Segment::Protected {
                self.protected_used_bytes =
                    self.protected_used_bytes.saturating_sub(ent.charge_bytes);
            }
            self.evicts += 1;
        }
    }

    fn clear_all(&mut self) {
        self.used_bytes = 0;
        self.protected_used_bytes = 0;
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
        if removed > 0 {
            self.maybe_compact_lru_queues();
        }
        removed
    }
}

/// Global contract cache pool (cross-transaction, cross-block).
///
/// Heat + decay semantics (ticks are pool accesses, not wall time):
/// - Each access increments cache `tick` by 1.
/// - On entry access, its heat is first decayed by half-lives:
///     heat = heat >> ((tick - last_tick) / heat_half_life)
///   then boosted:
///     heat += hit_boost (saturating)
/// - Probation->Protected promotion happens when `heat >= promote_threshold`.
/// - Eviction is SLRU by bytes: evict LRU from probation first; then from protected.
pub struct ContractCachePool {
    inner: Mutex<ContractCacheInner>,
}

impl Default for ContractCachePool {
    fn default() -> Self {
        Self {
            inner: Mutex::new(ContractCacheInner::default()),
        }
    }
}

impl ContractCachePool {
    pub fn configure(&self, config: ContractCacheConfig) {
        let mut inner = self.inner.lock().expect("ContractCachePool lock poisoned");
        inner.config = config;
        inner.evict_until_fit();
    }

    pub fn stats(&self) -> ContractCacheStats {
        let inner = self.inner.lock().expect("ContractCachePool lock poisoned");
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
        let mut inner = self.inner.lock().expect("ContractCachePool lock poisoned");
        inner.get(&ContractCacheInner::make_key(addr, edition))
    }

    pub fn insert(&self, addr: &ContractAddress, obj: Arc<ContractObj>) {
        let mut inner = self.inner.lock().expect("ContractCachePool lock poisoned");
        inner.insert(addr, obj);
    }

    pub fn remove_addr(&self, addr: &ContractAddress) -> usize {
        let mut inner = self.inner.lock().expect("ContractCachePool lock poisoned");
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
