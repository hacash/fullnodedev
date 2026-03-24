




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

    fn config(&self) -> Box<dyn Any> {
        Box::new(self.cnf.clone())
    }

    fn tx_submit(&self, eng: &dyn EngineRead, tx: &TxPkg) -> Rerr {
        impl_tx_submit(self, eng, tx)
    }

    fn blk_found_pending(&self, curblk: &dyn BlockRead, body: &Vec<u8>, sto: &dyn Store) -> Option<RetBlkFound> {
        impl_blk_found_pending(self, curblk, body, sto)
    }

    fn blk_found(&self, curblk: &dyn BlockRead, body: &Vec<u8>, sto: &dyn Store ) -> Rerr {
        impl_blk_found(self, curblk, body, sto)
    }

    fn blk_verify(&self, curblk: &dyn BlockRead, prevblk: &dyn BlockRead, sto: &dyn Store) -> Rerr {
        impl_blk_verify(self, curblk, prevblk, sto)
    }

    fn blk_insert(&self, curblk: &BlkPkg, sta: &dyn State, prev: &dyn State) -> Rerr {
        impl_blk_insert(self, curblk, sta, prev)
    }

    fn genesis_block(&self) -> Arc<dyn Block> {
        self.genesis_block.clone()
    }

    fn initialize(&self, sta: &mut dyn State) -> Rerr {
        do_initialize(self, sta)
    }

    // <dyn Block> == BlockV1
    fn packing_next_block(&self, eng: &dyn EngineRead, tp: &dyn TxPool) -> Box<dyn Any> {
        impl_packing_next_block(self, eng, tp)
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


    /*
    fn coinbase(&self, hei: u64, tx: &dyn TransactionRead) -> Rerr {
        verify_coinbase(hei, tx)
    }
    */

    fn tx_pool_group(&self, tx: &TxPkg) -> usize {
        let mut group_id =  TXGID_NORMAL;
        if let Some(..) = action::pickout_diamond_mint_action(tx.tx_read()) {
            group_id = TXGID_DIAMINT;
        }
        group_id
    }


    fn tx_pool_refresh(&self, eng: &dyn EngineRead, txp: &dyn TxPool, txs: Vec<Hash>, blkhei: u64) {
        impl_tx_pool_refresh(self, eng, txp, txs, blkhei)
    }

    fn exit(&self) {
        let mut biddings = self.bidding_prove.lock().unwrap();
        biddings.stop = true;
    }
}
