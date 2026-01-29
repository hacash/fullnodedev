
impl TxGroup {
    
    fn clear(&mut self) {
        self.txpkgs.clear()
    }

    fn remove(&mut self, txhx: &Hash) -> Option<TxPkg> {
        let Some(rmid) = self.search(txhx) else {
            return None
        };
        // remove
        Some(self.txpkgs.remove(rmid))
    }

    // remove out txs
    fn drain(&mut self, hxst: &mut HashSet<Hash>) -> Vec<TxPkg> {
        let mut res = vec![];
        let hxs: Vec<Hash> = hxst.iter().map(|a|a.clone()).collect();
        for hx in hxs {
            if let Some(txp) = self.remove(&hx) {
                hxst.remove(&hx);
                res.push(txp);
            }
        }
        res
    }


    // delete if false
    fn retain(&mut self, f: &mut dyn FnMut(&TxPkg)->bool) {
        self.txpkgs.retain(f)
    }


    fn delete(&mut self, txhxs: &[Hash]) {
        for hx in txhxs {
            self.del_one(hx);
        }
    }

    // delete one tx
    fn del_one(&mut self, hx: &Hash) -> bool {
        let mut rmidx: usize = 0;
        for tx in self.txpkgs.iter() {
            if tx.hash == *hx {
                self.txpkgs.remove(rmidx);
                return true
            }
            rmidx += 1;
        }
        // not find
        false
    }
    

}
