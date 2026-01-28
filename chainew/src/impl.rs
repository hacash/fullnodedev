
impl Engine for ChainEngine {
    fn as_read(&self) -> &dyn EngineRead { self }
    
    fn discover(&self, blk: BlkPkg) -> Rerr {
        let _isrtlock = inserting_lock(self, ISRT_STAT_DISCOVER,
            "the blockchain is syncing and cannot insert newly discovered block"
        )?;
        // let _lk = self.isrtlk.lock().unwrap();
        let mut tree = self.tree.write().unwrap();
        let rid = insert_by(self, tree.deref_mut(), blk)?;
        let became_head = rid.head_change.is_some();
        roll_by(self, rid)?;

        // Update runtime caches only after the block is fully accepted and rolled.
        // Also only track canonical(head) progression to avoid pollution by side-chain/invalid blocks.
        if became_head {
            let head = tree.head.block.as_read();
            if self.cnf.recent_blocks {
                record_recent(self, head);
            }
            if self.cnf.average_fee_purity {
                record_avgfee(self, head);
            }
        }
        Ok(())
    }

    fn synchronize(&self, datas: Vec<u8>) -> Rerr {
        synchronize(self, datas.into(), BlkOrigin::Sync)
    }

    fn exit(&self) { 
        print!("Engine ::  fn exit(&self) start");
        let _lk = self.rebuilding.lock().unwrap();
        print!("Engine ::  fn exit(&self) end");
        self.minter.exit();
        self.scaner.exit();
    }
}

impl EngineRead for ChainEngine {
    fn config(&self) -> &EngineConf { &self.cnf }

    fn latest_block(&self) -> Arc<dyn Block> {
        self.tree.read().unwrap().head.block.clone()
    }
    
    fn store(&self) -> Arc<dyn Store> { self.store.clone() }
    
    fn state(&self) -> Arc<Box<dyn State>> {
        self.tree.read().unwrap().head.state.clone()
    }

    fn minter(&self) -> &dyn Minter { self.minter.as_ref() }
    
    fn logs(&self) -> Arc<dyn Logs> { self.logs.clone() }
    
    fn fork_sub_state(&self) -> Box<dyn State> {
         let tree = self.tree.read().unwrap();
         tree.head.state.fork_sub(Arc::downgrade(&tree.head.state))
    }

    fn recent_blocks(&self) -> Vec<Arc<RecentBlockInfo>> { 
        self.recent_blocks.lock().unwrap().iter().cloned().collect()
    }
    
    fn try_execute_tx(&self, tx: &dyn TransactionRead) -> Rerr {
        let height = self.latest_block().height().uint() + 1;
        self.try_execute_tx_by(tx, height, &mut self.fork_sub_state())
    }

    fn try_execute_tx_by(&self, tx: &dyn TransactionRead, pd_hei: u64, sub_state: &mut Box<dyn State>) -> Rerr {
        try_execute_tx_by(self, tx, pd_hei, sub_state)
    }

    fn average_fee_purity(&self) -> u64 {
        let avgfs = self.avgfees.lock().unwrap();
        let al = avgfs.len();
        if al == 0 {
            return self.cnf.lowest_fee_purity
        }
        let ttn: u64 = avgfs.iter().sum();
        ttn / avgfs.len() as u64
    }
}
