const HXS: usize = 32; // hash size

#[derive(Clone)]
pub struct DifficultyGnr {
    cnf: MintConf,
    block_caches: Arc<Mutex<HashMap<u64, (u64, u32, [u8; HXS])>>>, // height => (time, diffhx)
}

impl DifficultyGnr {
    pub fn new(cnf: MintConf) -> DifficultyGnr {
        DifficultyGnr {
            cnf: cnf,
            block_caches: Arc::default(),
        }
    }
}

impl DifficultyGnr {
    pub fn req_cycle_block(&self, hei: u64, sto: &dyn Store) -> (u64, u32, [u8; HXS]) {
        let cylnum = self.cnf.difficulty_adjust_blocks; // 288
        if hei < cylnum {
            let cyltime = genesis::genesis_block().timestamp().uint();
            let diffcty = genesis::genesis_block().difficulty().uint();
            let diffhx = u32_to_hash(diffcty);
            return (cyltime, diffcty, diffhx);
        }
        let cylhei = hei / cylnum * cylnum;
        {
            let cache = self.block_caches.lock().unwrap();
            if let Some(blk_time) = cache.get(&cylhei) {
                return *blk_time; // find in cache
            }
        }
        // read from database
        let (_, blkdts) = sto
            .block_data_by_height(&BlockHeight::from(cylhei))
            .unwrap();
        let intro = BlockIntro::must(&blkdts);
        // get time
        let cyltime = intro.timestamp().uint();
        let diffcty = intro.difficulty().uint();
        let diffhx = u32_to_hash(diffcty);
        let ccitem = (cyltime, diffcty, diffhx);
        let mut cache = self.block_caches.lock().unwrap();
        if let Some(blk_time) = cache.get(&cylhei) {
            return *blk_time;
        }
        cache.insert(cylhei, ccitem);
        if cache.len() as u64 > cylnum {
            cache.clear(); // clear
        }
        // ok
        ccitem
    }

    /*
     *
     */
    pub fn target(
        &self,
        mcnf: &MintConf,
        prevdiff: u32,
        prevblkt: u64,
        hei: u64,
        sto: &dyn Store,
    ) -> (u32, [u8; 32], BigUint) {
        let cylnum = self.cnf.difficulty_adjust_blocks;
        if hei < cylnum * 2 {
            let dn = LOWEST_DIFFICULTY;
            return (dn, u32_to_hash(dn), u32_to_biguint(dn));
        }
        if hei % cylnum != 0 {
            let hx = u32_to_hash(prevdiff);
            return (prevdiff, hx, hash_to_biguint(&hx));
        }
        // count time
        let blk_span = self.cnf.each_block_target_time;
        let target_time_span = cylnum * blk_span; // 288 * 300
        let (prevcltime, _, _) = self.req_cycle_block(hei - cylnum, sto);
        let mut real_time_span = blk_span + prevblkt - prevcltime; // +300: 287+1block
        if mcnf.is_mainnet() && hei < cylnum * 450 {
            // in mainnet chain id = 0
            // -300 = 287block, compatible history code
            real_time_span -= blk_span;
        }
        let minsecs = target_time_span / 4;
        let maxsecs = target_time_span * 4;
        if real_time_span < minsecs {
            real_time_span = minsecs;
        } else if real_time_span > maxsecs {
            real_time_span = maxsecs;
        }
        // calculate
        let prevbign = u32_to_biguint(prevdiff);
        let targetbign = prevbign * BigUint::from(real_time_span) / BigUint::from(target_time_span);
        let tarhash = biguint_to_hash(&targetbign);
        let tarnum = hash_to_u32(&tarhash);
        (tarnum, tarhash, targetbign)
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

    struct CountingStore {
        reads: AtomicUsize,
        data: Vec<u8>,
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
        fn block_data_by_height(&self, _: &BlockHeight) -> Option<(Hash, Vec<u8>)> {
            self.reads.fetch_add(1, Ordering::SeqCst);
            Some((Hash::default(), self.data.clone()))
        }
    }

    #[test]
    fn req_cycle_block_reads_disk_once_after_cache_warmup() {
        let _setup = protocol::setup::install_scoped_for_test(
            protocol::setup::SetupBuilder::new()
                .block_hasher(x16rs::block_hash)
                .action_register(protocol::action::register)
                .build()
                .unwrap(),
        );
        let dgnr = DifficultyGnr::new(MintConf::new(&IniObj::new()));
        let store = CountingStore {
            reads: AtomicUsize::new(0),
            data: genesis::genesis_block().intro.serialize(),
        };

        let first = dgnr.req_cycle_block(288, &store);
        let second = dgnr.req_cycle_block(288, &store);

        assert_eq!(first, second);
        assert_eq!(store.reads.load(Ordering::SeqCst), 1);
    }
}
