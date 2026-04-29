


/*******************************************************/

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RetBlkFound {
    Normal,
    PendingCached,
    Reject,
}

pub trait BlockIntroSource: Send + Sync {
    fn cache_height_limit(&self) -> u64;
    fn block_intro(&self, hei: u64) -> Option<Box<dyn BlockRead>>;
}

/// Returned when low-bid shadow storage is full; discover must not broadcast this block.
pub const LOW_BID_CACHE_FULL_ERR: &str = "mint.low_bid.cache_full";


pub trait Minter : Send + Sync {
    // static material
    fn genesis_block(&self) -> Arc<dyn Block> { never!() }
    fn config(&self) -> Box<dyn Any> { never!() }
    fn initialize(&self, _: &mut dyn State) -> Rerr { Ok(()) }

    // tx flow
    // Runs after generic tx execution precheck and before txpool insertion.
    fn tx_submit(&self, _: &dyn EngineRead, _: &TxPkg) -> Rerr { Ok(()) }
    fn tx_pool_group(&self, _: &TxPkg) -> usize { 0 }
    // Runs only after a discovered block has been fully accepted.
    fn tx_pool_refresh(&self, _: &dyn EngineRead, _: &dyn TxPool, _txs: Vec<Hash>, _blkhei: u64) {}

    // block flow
    // Earliest shadow-chain forwarding gate. Must stay before height/root/head checks.
    fn blk_found(&self, _: &dyn BlockRead, _: &Vec<u8>, _: &dyn Store) -> Option<RetBlkFound> { None }
    // Header quick gate. Runs after height/root/head checks and before full block parse.
    fn blk_arrive(&self, _: &dyn BlockRead, _: &Vec<u8>, _: &dyn Store) -> Rerr { Ok(()) }
    // Full pre-exec block check. Runs before generic block verification and block execution.
    fn blk_verify(&self, _: &dyn BlockRead, _prev: &dyn BlockRead, _: &dyn BlockIntroSource) -> Rerr { Ok(()) }
    // Final gate. Runs after block execution and before forktree insertion.
    fn blk_insert(&self, _: &BlkPkg, _sub: &dyn State, _prev: &dyn State) -> Rerr { Ok(()) }
    // Stable-root callback. Runs after root/head roll is fully committed.
    fn blk_root_roll(&self, _: &dyn BlockRead, _: &dyn Store) {}

    // block build
    fn block_reward(&self, _: u64) -> u64 { 0 }
    fn preview_next_difficulty(&self, _: &dyn BlockRead, _: u64, _: &dyn BlockIntroSource) -> Option<(u32, [u8; 32], BigUint)> { None }
    fn packing_next_block(&self, _: &dyn EngineRead, _: &dyn TxPool) -> Box<dyn Any> { never!() } // BlockV1

    // runtime
    fn start(&self, _: Worker) {}
    fn bind_engine(&self, _: Arc<dyn Engine>) {}
    fn p2p_on_connect(&self, _: Arc<dyn NPeer>, _: Arc<dyn Engine>, _: Arc<dyn TxPool>) -> Rerr { Ok(()) }
    fn exit(&self) {}
}
