

impl HNoder for HacashNode {


    fn start(&self, worker: Worker) {
        self.do_start(worker)
    }


    fn submit_transaction(&self, txpkg: &TxPkg, in_async: bool, only_insert_txpool: bool) -> Rerr {
        let txread = txpkg.objc.as_read();
        self.engine.try_execute_tx(txread)?;
        if only_insert_txpool {
            // Direct insert to txpool, no channel, no broadcast
            let minter = self.engine.minter();
            minter.tx_submit(self.engine.as_read(), txpkg)?;
            let _ = self.txpool.insert_by(txpkg.clone(), &|tx| minter.tx_pool_group(tx));
            return Ok(());
        }
        let msghdl = self.msghdl.clone();
        let txbody = txpkg.data.to_vec();
        let runobj = async move {
            msghdl.submit_transaction(txbody).await;
        };
        if in_async {
            tokio::spawn(runobj);
        }else{
            new_current_thread_tokio_rt().block_on(runobj);
        }
        Ok(())
    }

    fn submit_block(&self, blkpkg: &BlkPkg, in_async: bool) -> Rerr {
        // NOT do any check
        // insert
        let msghdl = self.msghdl.clone();
        let blkbody = blkpkg.data.to_vec();
        let runobj = async move {
            msghdl.submit_block(blkbody).await;
        };
        if in_async {
            tokio::spawn(runobj);
        }else{
            new_current_thread_tokio_rt().block_on(runobj);
        }
        Ok(())
    }

    fn engine(&self) -> Arc<dyn Engine> {
        self.engine.clone()
    }

    fn txpool(&self) -> Arc<dyn TxPool> {
        self.txpool.clone()
    }

    fn all_peer_prints(&self) -> Vec<String> { 
        self.p2p.all_peer_prints()
    }

    fn exit(&self) {
        self.msghdl.exit();
        self.p2p.exit();
        self.engine.exit();
        println!("[Node] network exit.");
        // wait something to finish
        // std::thread::sleep(std::time::Duration::from_secs_f32(0.5));
    }

}
