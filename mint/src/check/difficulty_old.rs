impl DifficultyGnr {
    pub(crate) fn req_cycle_block(&self, hei: u64, sto: &dyn Store, trc: Option<&ForkTrace>) -> CachedBlockIntro {
        let cylnum = self.cnf.difficulty_adjust_blocks;
        if hei < cylnum {
            return self.genesis_cache_item()
        }
        let cylhei = hei / cylnum * cylnum;
        self.req_block_intro(cylhei, hei, sto, trc)
    }

    fn target_legacy(
        &self,
        mcnf: &MintConf,
        prevdiff: u32,
        prevblkt: u64,
        hei: u64,
        sto: &dyn Store,
        trc: Option<&ForkTrace>,
    ) -> DifficultyTarget {
        let cylnum = self.cnf.difficulty_adjust_blocks;
        if hei < cylnum * 2 {
            return self.target_bootstrap()
        }
        if hei % cylnum != 0 {
            return DifficultyTarget::from_num(prevdiff)
        }
        let blk_span = self.cnf.each_block_target_time;
        let target_time_span = cylnum * blk_span;
        let (prevcltime, _, _) = self.req_cycle_block(hei - cylnum, sto, trc);
        let mut real_time_span = blk_span + prevblkt - prevcltime;
        if mcnf.is_mainnet() && hei < cylnum * 450 {
            real_time_span -= blk_span;
        }
        let minsecs = target_time_span / 4;
        let maxsecs = target_time_span * 4;
        if real_time_span < minsecs {
            real_time_span = minsecs;
        } else if real_time_span > maxsecs {
            real_time_span = maxsecs;
        }
        let prevbign = u32_to_biguint(prevdiff);
        let targetbign = prevbign * BigUint::from(real_time_span) / BigUint::from(target_time_span);
        DifficultyTarget::from_big(targetbign)
    }
}
