use super::*;

pub struct HacashNode {
    runtime: Arc<NodeRuntime>,
}

impl HacashNode {
    pub fn open(ini: &IniObj, txpool: Arc<dyn TxPool>, engine: Arc<dyn Engine>) -> Self {
        Self {
            runtime: Arc::new(NodeRuntime::open(ini, txpool, engine)),
        }
    }
}

impl HNoder for HacashNode {
    fn start(&self, worker: Worker) {
        self.runtime.start(worker)
    }

    fn submit_transaction(&self, txpkg: &TxPkg, in_async: bool, only_insert_txpool: bool) -> Rerr {
        self.runtime.submit_transaction(txpkg, in_async, only_insert_txpool)
    }

    fn submit_block(&self, blkpkg: &BlkPkg, in_async: bool) -> Rerr {
        self.runtime.submit_block(blkpkg, in_async)
    }

    fn engine(&self) -> Arc<dyn Engine> {
        self.runtime.engine()
    }

    fn txpool(&self) -> Arc<dyn TxPool> {
        self.runtime.txpool()
    }

    fn all_peer_prints(&self) -> Vec<String> {
        self.runtime.all_peer_prints()
    }

    fn exit(&self) {
        self.runtime.exit()
    }
}
