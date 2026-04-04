use super::*;

impl NodeRuntime {
    pub fn submit_transaction_inner(&self, txpkg: &TxPkg, in_async: bool, only_insert_txpool: bool) -> Rerr {
        self.protocol.submit_transaction(txpkg, self.engine.clone(), self.txpool.clone(), in_async, only_insert_txpool)
    }

    pub fn submit_block_inner(&self, blkpkg: &BlkPkg, in_async: bool) -> Rerr {
        self.protocol.submit_block(blkpkg, in_async)
    }
}
