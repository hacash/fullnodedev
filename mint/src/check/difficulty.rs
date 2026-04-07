const HXS: usize = 32; // hash size
const DIFFICULTY_UPGRADE_EPOCH: u64 = 2563; // 738144 height

type CachedBlockIntro = (u64, u32, [u8; HXS]); // (time, diffnum, diffhash)

#[derive(Clone)]
pub struct DifficultyGnr {
    cnf: MintConf,
    block_caches: Arc<Mutex<HashMap<u64, CachedBlockIntro>>>,
    last_target_cache: Arc<Mutex<Option<(u64, u32, u64, [u8; HXS], BigUint)>>>, // (next_height, prevdiff, prevtime, tarhash, target)
}

impl DifficultyGnr {
    pub fn new(cnf: MintConf) -> DifficultyGnr {
        DifficultyGnr {
            cnf: cnf,
            block_caches: Arc::default(),
            last_target_cache: Arc::default(),
        }
    }
}

impl DifficultyGnr {
    fn upgrade_height(&self) -> u64 {
        if self.cnf.is_mainnet() {
            DIFFICULTY_UPGRADE_EPOCH * self.cnf.difficulty_adjust_blocks
        } else {
            1
        }
    }

    fn window_blocks(&self) -> u64 {
        self.cnf.difficulty_adjust_blocks
    }

    fn group_blocks(&self) -> u64 {
        self.cnf.difficulty_group_blocks
    }

    fn window_groups(&self) -> u64 {
        self.window_blocks() / self.group_blocks()
    }

    fn cache_blocks(&self) -> u64 {
        self.window_blocks() + self.group_blocks() * 3
    }

    pub fn is_upgrade_height(&self, hei: u64) -> bool {
        hei >= self.upgrade_height()
    }

    fn use_bootstrap_rule(&self, hei: u64) -> bool {
        hei <= self.window_blocks() + 1
    }

    pub fn cache_block_intro(&self, blk: &dyn BlockRead) {
        let item = (blk.timestamp().uint(), blk.difficulty().uint(), u32_to_hash(blk.difficulty().uint()));
        let mut cache = self.block_caches.lock().unwrap();
        cache.insert(blk.height().uint(), item);
        self.prune_cache_after_insert(&mut cache, blk.height().uint());
        let mut last = self.last_target_cache.lock().unwrap();
        *last = None;
    }

    pub fn cache_root_block_intro(&self, blk: &dyn BlockRead) {
        self.cache_block_intro(blk)
    }

    fn genesis_cache_item(&self) -> CachedBlockIntro {
        let time = genesis::genesis_block().timestamp().uint();
        let diff = genesis::genesis_block().difficulty().uint();
        (time, diff, u32_to_hash(diff))
    }

    fn req_block_intro(&self, hei: u64, reqhei: u64, sto: &dyn Store) -> CachedBlockIntro {
        if hei == 0 {
            return self.genesis_cache_item()
        }
        let root_height = sto.status().root_height.uint();
        {
            let cache = self.block_caches.lock().unwrap();
            if let Some(blk_time) = cache.get(&hei) {
                if hei > root_height || reqhei <= root_height {
                    return *blk_time;
                }
            }
        }
        let Some((hx, blkdts)) = sto.block_data_by_height(&BlockHeight::from(hei)) else {
            self.log_cycle_block_miss(reqhei, hei, self.cnf.difficulty_adjust_blocks, sto);
            panic!(
                "difficulty block missing: request_height={} block_height={}",
                reqhei,
                hei,
            );
        };
        let intro = match BlockIntro::build(&blkdts) {
            Ok(v) => v,
            Err(e) => {
                self.log_cycle_block_parse_failure(reqhei, hei, self.cnf.difficulty_adjust_blocks, &hx, &blkdts, &e);
                panic!(
                    "difficulty block parse failed: request_height={} block_height={} hash={}",
                    reqhei,
                    hei,
                    hx.half(),
                );
            }
        };
        let diffcty = intro.difficulty().uint();
        let item = (intro.timestamp().uint(), diffcty, u32_to_hash(diffcty));
        let mut cache = self.block_caches.lock().unwrap();
        if let Some(blk_time) = cache.get(&hei) {
            return *blk_time;
        }
        cache.insert(hei, item);
        self.prune_cache_after_insert(&mut cache, hei);
        item
    }

    fn prune_cache_after_insert(&self, cache: &mut HashMap<u64, CachedBlockIntro>, newest: u64) {
        if cache.len() as u64 <= self.cache_blocks() {
            return;
        }
        let min_keep = newest.saturating_sub(self.cache_blocks() - 1);
        cache.retain(|hei, _| *hei >= min_keep);
    }

    fn target_bootstrap(&self) -> DifficultyTarget {
        DifficultyTarget::from_num(LOWEST_DIFFICULTY)
    }

    fn target_weighted_sliding(
        &self,
        prevdiff: u32,
        prevblkt: u64,
        hei: u64,
        sto: &dyn Store,
    ) -> DifficultyTarget {
        if let Some((cached_hei, cached_diff, cached_time, cached_hash, cached_target)) = &*self.last_target_cache.lock().unwrap() {
            if *cached_hei == hei && *cached_diff == prevdiff && *cached_time == prevblkt {
                return DifficultyTarget::new(hash_to_u32(cached_hash), *cached_hash, cached_target.clone());
            }
        }
        let prevbign = u32_to_biguint(prevdiff);
        let mut observed: u128 = 0;
        let mut expected: u128 = 0;
        let group_target = (self.cnf.each_block_target_time * self.group_blocks()) as u128;
        let mut bound = hei - self.window_blocks() - 1;
        let mut prev_time = self.req_block_intro(bound, hei, sto).0;
        let last_group = self.window_groups() - 1;
        for i in 0..self.window_groups() {
            let next_time = if i == last_group {
                prevblkt
            } else {
                bound += self.group_blocks();
                self.req_block_intro(bound, hei, sto).0
            };
            let weight = (i + 1) as u128;
            observed += (next_time.saturating_sub(prev_time) as u128) * weight;
            expected += group_target * weight;
            prev_time = next_time;
        }
        let targetbign = clamp_target_half_double(&prevbign, scale_target_by_ratio(&prevbign, observed, expected));
        let target = DifficultyTarget::from_big(targetbign);
        let mut last = self.last_target_cache.lock().unwrap();
        *last = Some((hei, prevdiff, prevblkt, target.hash, target.big.clone()));
        target
    }

    fn log_cycle_block_miss(&self, reqhei: u64, cylhei: u64, cylnum: u64, sto: &dyn Store) {
        let stat = sto.status();
        let has_cycle_hash = sto.block_hash(&BlockHeight::from(cylhei));
        let prev_hash = cylhei
            .checked_sub(1)
            .and_then(|hei| sto.block_hash(&BlockHeight::from(hei)));
        let next_hash = sto.block_hash(&BlockHeight::from(cylhei + 1));
        eprintln!(
            "[Difficulty] cycle block lookup failed req_height={} cycle_height={} cycle_blocks={} root_height={} last_height={} cycle_hash={} prev_hash={} next_hash={} thread={:?}",
            reqhei,
            cylhei,
            cylnum,
            stat.root_height.uint(),
            stat.last_height.uint(),
            has_cycle_hash.as_ref().map(|v| format!("{}", v.half())).unwrap_or_else(|| "<missing>".to_string()),
            prev_hash.as_ref().map(|v| format!("{}", v.half())).unwrap_or_else(|| "<missing>".to_string()),
            next_hash.as_ref().map(|v| format!("{}", v.half())).unwrap_or_else(|| "<missing>".to_string()),
            std::thread::current().id(),
        );
    }

    fn log_cycle_block_parse_failure(
        &self,
        reqhei: u64,
        cylhei: u64,
        cylnum: u64,
        hx: &Hash,
        blkdts: &[u8],
        err: &String,
    ) {
        eprintln!(
            "[Difficulty] cycle block parse failed req_height={} cycle_height={} cycle_blocks={} hash={} data_len={} err={}",
            reqhei,
            cylhei,
            cylnum,
            hx.half(),
            blkdts.len(),
            err,
        );
    }

    pub fn target(
        &self,
        mcnf: &MintConf,
        prevdiff: u32,
        prevblkt: u64,
        hei: u64,
        sto: &dyn Store,
    ) -> (u32, [u8; 32], BigUint) {
        if mcnf.is_mainnet() && !self.is_upgrade_height(hei) {
            return self.target_legacy(mcnf, prevdiff, prevblkt, hei, sto).into_tuple()
        }
        if self.use_bootstrap_rule(hei) {
            return self.target_bootstrap().into_tuple()
        }
        self.target_weighted_sliding(prevdiff, prevblkt, hei, sto).into_tuple()
    }
}

impl HacashMinter {
    fn next_difficulty(&self, prev: &dyn BlockRead, sto: &dyn Store) -> (u32, [u8; 32], BigUint) {
        let pdif = prev.difficulty().uint();
        let ptim = prev.timestamp().uint();
        let nhei = prev.height().uint() + 1;
        self.difficulty.target(&self.cnf, pdif, ptim, nhei, sto)
    }
}

#[cfg(test)]
mod difficulty_tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    fn scoped_protocol_setup() -> protocol::setup::TestSetupScopeGuard {
        let mut setup = protocol::setup::new_standard_protocol_setup(x16rs::block_hash);
        crate::setup::register_protocol_extensions(&mut setup);
        protocol::setup::install_test_scope(setup)
    }

    struct CountingStore {
        reads: AtomicUsize,
        blocks: HashMap<u64, Vec<u8>>,
    }

    impl Store for CountingStore {
        fn status(&self) -> ChainStatus {
            ChainStatus::default()
        }
        fn save_block_data(&self, _: &Hash, _: &Vec<u8>) {}
        fn save_block_hash(&self, _: &BlockHeight, _: &Hash) {}
        fn save_block_hash_path(&self, _: &dyn MemDB) {}
        fn save_batch(&self, _: &dyn MemDB) {}
        fn block_data(&self, _: &Hash) -> Option<Vec<u8>> {
            None
        }
        fn block_hash(&self, _: &BlockHeight) -> Option<Hash> {
            None
        }
        fn block_data_by_height(&self, hei: &BlockHeight) -> Option<(Hash, Vec<u8>)> {
            self.reads.fetch_add(1, Ordering::SeqCst);
            self.blocks.get(&hei.uint()).cloned().map(|v| (Hash::default(), v))
        }
    }

    fn build_intro(height: u64, timestamp: u64, difficulty: u32) -> Vec<u8> {
        BlockIntro {
            head: BlockHead {
                version: Uint1::from(1),
                height: BlockHeight::from(height),
                timestamp: Timestamp::from(timestamp),
                prevhash: Hash::default(),
                mrklroot: Hash::default(),
                transaction_count: Uint4::default(),
            },
            meta: BlockMeta {
                nonce: Uint4::default(),
                difficulty: Uint4::from(difficulty),
                witness_stage: Fixed2::default(),
            },
        }.serialize()
    }

    #[test]
    fn req_cycle_block_reads_disk_once_after_cache_warmup() {
        let _setup = scoped_protocol_setup();
        let dgnr = DifficultyGnr::new(MintConf::new(&IniObj::new()));
        let mut blocks = HashMap::new();
        blocks.insert(288, build_intro(288, 1000, LOWEST_DIFFICULTY));
        let store = CountingStore {
            reads: AtomicUsize::new(0),
            blocks,
        };

        let first = dgnr.req_cycle_block(288, &store);
        let second = dgnr.req_cycle_block(288, &store);

        assert_eq!(first, second);
        assert_eq!(store.reads.load(Ordering::SeqCst), 1);
    }

    fn new_test_dgnr() -> DifficultyGnr {
        DifficultyGnr::new(MintConf::new(&IniObj::new()))
    }

    fn new_test_dgnr_with(adjust_blocks: u64, group_blocks: u64, target_time: u64, chain_id: u64) -> DifficultyGnr {
        let mut ini = IniObj::new();
        let mut mint = HashMap::new();
        mint.insert("chain_id".to_string(), Some(chain_id.to_string()));
        mint.insert("difficulty_adjust_blocks".to_string(), Some(adjust_blocks.to_string()));
        mint.insert("difficulty_group_blocks".to_string(), Some(group_blocks.to_string()));
        mint.insert("each_block_target_time".to_string(), Some(target_time.to_string()));
        ini.insert("mint".to_string(), mint);
        DifficultyGnr::new(MintConf::new(&ini))
    }

    fn test_upgrade_height(dgnr: &DifficultyGnr) -> u64 {
        dgnr.upgrade_height()
    }

    #[test]
    fn weighted_window_prefers_memory_cache_before_store() {
        let _setup = scoped_protocol_setup();
        let dgnr = new_test_dgnr();
        let mut blocks = HashMap::new();
        let upgrade_height = test_upgrade_height(&dgnr);
        let start = upgrade_height - dgnr.window_blocks() - 1;
        for i in 0..=dgnr.window_blocks() {
            let hei = start + i;
            blocks.insert(hei, build_intro(hei, 10_000 + i * 300, LOWEST_DIFFICULTY));
        }
        let store = CountingStore {
            reads: AtomicUsize::new(0),
            blocks,
        };

        for i in 0..dgnr.cache_blocks() {
            let hei = upgrade_height - dgnr.cache_blocks() + i;
            let intro = BlockIntro::build(&build_intro(hei, 20_000 + i * 300, LOWEST_DIFFICULTY)).unwrap();
            dgnr.cache_block_intro(&intro);
        }
        let reads_before = store.reads.load(Ordering::SeqCst);
        let _ = dgnr.req_block_intro(upgrade_height - 1, upgrade_height, &store);
        let reads_after = store.reads.load(Ordering::SeqCst);
        assert_eq!(reads_before, reads_after);
    }

    #[test]
    fn weighted_window_clamps_to_half_and_double() {
        let _setup = scoped_protocol_setup();
        let prevdiff = LOWEST_DIFFICULTY - 1024;
        let dgnr_fast = new_test_dgnr();
        let upgrade_height = test_upgrade_height(&dgnr_fast);
        let start = upgrade_height - dgnr_fast.window_blocks() - 1;

        let mut fast_blocks = HashMap::new();
        for i in 0..=dgnr_fast.window_blocks() {
            let hei = start + i;
            fast_blocks.insert(hei, build_intro(hei, 10_000 + i, prevdiff));
        }
        let fast_store = CountingStore {
            reads: AtomicUsize::new(0),
            blocks: fast_blocks,
        };
        let prev_target = u32_to_biguint(prevdiff);
        let fast_target = dgnr_fast.target_weighted_sliding(prevdiff, 10_000 + dgnr_fast.window_blocks(), upgrade_height, &fast_store).big;
        assert!(fast_target <= prev_target.clone());
        assert!(fast_target >= prev_target.clone() / BigUint::from(2u8));

        let dgnr_slow = new_test_dgnr();
        let mut slow_blocks = HashMap::new();
        for i in 0..=dgnr_slow.window_blocks() {
            let hei = start + i;
            slow_blocks.insert(hei, build_intro(hei, 10_000 + i * 10_000, prevdiff));
        }
        let slow_store = CountingStore {
            reads: AtomicUsize::new(0),
            blocks: slow_blocks,
        };
        let slow_target = dgnr_slow.target_weighted_sliding(prevdiff, 10_000 + dgnr_slow.window_blocks() * 10_000, upgrade_height, &slow_store).big;
        assert!(slow_target >= prev_target.clone());
        assert!(slow_target <= prev_target * BigUint::from(2u8));
    }

    #[test]
    fn weighted_window_scales_with_config_changes() {
        let _setup = scoped_protocol_setup();
        let prevdiff = LOWEST_DIFFICULTY - 1024;
        let cases = [
            (288u64, 4u64, 300u64),
            (144u64, 4u64, 150u64),
            (120u64, 5u64, 60u64),
        ];

        for (adjust_blocks, group_blocks, target_time) in cases {
            let dgnr = new_test_dgnr_with(adjust_blocks, group_blocks, target_time, 9);
            let next_height = dgnr.window_blocks() + 2;
            let start = 1;
            let mut blocks = HashMap::new();
            for i in 0..=dgnr.window_blocks() {
                let hei = start + i;
                blocks.insert(hei, build_intro(hei, 50_000 + i * target_time, prevdiff));
            }
            let store = CountingStore {
                reads: AtomicUsize::new(0),
                blocks,
            };
            let prev_time = 50_000 + dgnr.window_blocks() * target_time;
            let target = dgnr.target_weighted_sliding(prevdiff, prev_time, next_height, &store);
            let prev_target = u32_to_biguint(prevdiff);
            assert!(target.big <= prev_target.clone() * BigUint::from(2u8));
            assert!(target.big >= prev_target.clone() / BigUint::from(2u8));
            let ratio_num = if target.big > prev_target.clone() { target.big.clone() } else { prev_target.clone() };
            let ratio_den = if target.big > prev_target.clone() { prev_target.clone() } else { target.big.clone() };
            assert!(ratio_num <= ratio_den * BigUint::from(2u8));
        }
    }

    #[test]
    fn weighted_window_keeps_target_when_observed_equals_expected() {
        let _setup = scoped_protocol_setup();
        let prevdiff = LOWEST_DIFFICULTY - 1024;
        let dgnr = new_test_dgnr_with(120, 5, 60, 9);
        let next_height = dgnr.window_blocks() + 2;
        let mut blocks = HashMap::new();
        for i in 0..=dgnr.window_blocks() {
            let hei = 1 + i;
            blocks.insert(hei, build_intro(hei, 1000 + i * dgnr.cnf.each_block_target_time, prevdiff));
        }
        let store = CountingStore { reads: AtomicUsize::new(0), blocks };
        let prev_time = 1000 + dgnr.window_blocks() * dgnr.cnf.each_block_target_time;
        let target = dgnr.target_weighted_sliding(prevdiff, prev_time, next_height, &store);
        assert_eq!(target.num, prevdiff);
    }

    #[test]
    fn weighted_window_latest_groups_have_larger_impact_than_oldest_groups() {
        let _setup = scoped_protocol_setup();
        let prevdiff = LOWEST_DIFFICULTY - 1024;
        let dgnr = new_test_dgnr();
        let next_height = dgnr.window_blocks() + 2;
        let base_time = 10_000u64;

        let mut oldest_blocks = HashMap::new();
        let mut newest_blocks = HashMap::new();
        for i in 0..=dgnr.window_blocks() {
            let hei = 1 + i;
            let mut ts = base_time + i * dgnr.cnf.each_block_target_time;
            if i == dgnr.group_blocks() {
                ts += dgnr.cnf.each_block_target_time;
            }
            oldest_blocks.insert(hei, build_intro(hei, ts, prevdiff));
            let mut ts2 = base_time + i * dgnr.cnf.each_block_target_time;
            if i == dgnr.window_blocks() {
                ts2 += dgnr.cnf.each_block_target_time;
            }
            newest_blocks.insert(hei, build_intro(hei, ts2, prevdiff));
        }
        let oldest_store = CountingStore { reads: AtomicUsize::new(0), blocks: oldest_blocks };
        let newest_store = CountingStore { reads: AtomicUsize::new(0), blocks: newest_blocks };
        let prev_time_oldest = base_time + dgnr.window_blocks() * dgnr.cnf.each_block_target_time;
        let prev_time_newest = prev_time_oldest + dgnr.cnf.each_block_target_time;
        let oldest = dgnr.target_weighted_sliding(prevdiff, prev_time_oldest, next_height, &oldest_store);
        let newest = dgnr.target_weighted_sliding(prevdiff, prev_time_newest, next_height, &newest_store);
        let prev_target = u32_to_biguint(prevdiff);
        let oldest_delta = if oldest.big > prev_target { oldest.big.clone() - prev_target.clone() } else { prev_target.clone() - oldest.big.clone() };
        let newest_delta = if newest.big > prev_target { newest.big.clone() - prev_target.clone() } else { prev_target.clone() - newest.big.clone() };
        assert!(newest_delta > oldest_delta);
    }

    #[test]
    fn weighted_window_uses_real_parent_time_for_last_group() {
        let _setup = scoped_protocol_setup();
        let prevdiff = LOWEST_DIFFICULTY - 1024;
        let dgnr = new_test_dgnr();
        let next_height = dgnr.window_blocks() + 2;
        let mut blocks = HashMap::new();
        for i in 0..=dgnr.window_blocks() {
            let hei = 1 + i;
            blocks.insert(hei, build_intro(hei, 10_000 + i * dgnr.cnf.each_block_target_time, prevdiff));
        }
        let store = CountingStore { reads: AtomicUsize::new(0), blocks };
        let canonical_prev = 10_000 + dgnr.window_blocks() * dgnr.cnf.each_block_target_time;
        let side_prev = canonical_prev + dgnr.cnf.each_block_target_time * 3;
        let canonical = dgnr.target_weighted_sliding(prevdiff, canonical_prev, next_height, &store);
        let side = dgnr.target_weighted_sliding(prevdiff, side_prev, next_height, &store);
        assert!(side.big > canonical.big);
    }
}
