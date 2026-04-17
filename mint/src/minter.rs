




#[allow(dead_code)]
pub struct HacashMinter {
    cnf: MintConf,
    difficulty: DifficultyGnr,
    genesis_block: Arc<dyn Block>,
    // check highest bidding
    bidding_prove: Arc<Mutex<BiddingProve>>,
}

impl HacashMinter {

    pub fn create(ini: &IniObj) -> Self {

        // create
        let cnf = MintConf::new(ini);
        let dgnr = DifficultyGnr::new(cnf.clone());
        Self {
            cnf: cnf,
            difficulty: dgnr,
            genesis_block: genesis::genesis_block_pkg().block_clone(),
            bidding_prove: Arc::new(Mutex::new(BiddingProve::new(usize::MAX))),
        }
    }

}


impl Minter for HacashMinter {

    fn genesis_block(&self) -> Arc<dyn Block> {
        self.genesis_block.clone()
    }

    fn config(&self) -> Box<dyn Any> {
        Box::new(self.cnf.clone())
    }

    fn initialize(&self, sta: &mut dyn State) -> Rerr {
        do_initialize(self, sta)
    }

    fn tx_submit(&self, eng: &dyn EngineRead, tx: &TxPkg) -> Rerr {
        impl_tx_submit(self, eng, tx)
    }

    fn tx_pool_group(&self, tx: &TxPkg) -> usize {
        impl_tx_pool_group(tx)
    }

    fn tx_pool_refresh(&self, eng: &dyn EngineRead, txp: &dyn TxPool, txs: Vec<Hash>, blkhei: u64) {
        impl_tx_pool_refresh(self, eng, txp, txs, blkhei)
    }

    fn blk_found(&self, curblk: &dyn BlockRead, body: &Vec<u8>, sto: &dyn Store) -> Option<RetBlkFound> {
        impl_blk_found(self, curblk, body, sto)
    }

    fn blk_arrive(&self, curblk: &dyn BlockRead, body: &Vec<u8>, sto: &dyn Store ) -> Rerr {
        impl_blk_arrive(self, curblk, body, sto)
    }

    fn blk_verify(&self, curblk: &dyn BlockRead, prevblk: &dyn BlockRead, sto: &dyn Store, trc: Option<&ForkTrace>) -> Rerr {
        impl_blk_verify(self, curblk, prevblk, sto, trc)
    }

    fn blk_insert(&self, curblk: &BlkPkg, sta: &dyn State, prev: &dyn State) -> Rerr {
        impl_blk_insert(self, curblk, sta, prev)
    }

    fn blk_root_roll(&self, rootblk: &dyn BlockRead, _: &dyn Store) {
        self.difficulty.cache_root_block_intro(rootblk)
    }

    // <dyn Block> == BlockV1
    fn packing_next_block(&self, eng: &dyn EngineRead, tp: &dyn TxPool) -> Box<dyn Any> {
        impl_packing_next_block(self, eng, tp)
    }

    fn start(&self, worker: Worker) {
        let start_loop = {
            let mut biddings = self.bidding_prove.lock().unwrap();
            biddings.start_loop()
        };
        if !start_loop {
            return;
        }
        let prove = self.bidding_prove.clone();
        low_bid_replay_loop(prove, worker);
    }

    fn bind_engine(&self, eng: Arc<dyn Engine>) {
        let mut biddings = self.bidding_prove.lock().unwrap();
        biddings.bind_engine(eng)
    }

    fn p2p_on_connect(&self, peer: Arc<dyn NPeer>, _: Arc<dyn Engine>, txpool: Arc<dyn TxPool>) -> Rerr {

        std::thread::spawn(move ||{
            if let Ok(Some(txp)) = txpool.first_at(TXGID_DIAMINT) {
                // send highest bidding diamond mint tx
                let _ = peer.send_msg_on_block(P2P_MSG_TX_SUBMIT, txp.data().to_vec());
            }
        });
        Ok(())
    }

    fn exit(&self) {
        let mut biddings = self.bidding_prove.lock().unwrap();
        biddings.stop = true;
    }
}
