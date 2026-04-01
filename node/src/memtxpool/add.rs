impl TxGroup {
    fn insert(&mut self, txp: TxPkg) -> Rerr {
        let feep = txp.fpur();
        let fee = txp.tx().fee().clone();
        if let Some((hid, hav)) = self.find(&txp.hash()) {
            let lsth = purity_or_fee! { self, txp, <=, hav };
            if lsth {
                return errf!("tx already exists in tx pool and its fee is higher");
            }
            self.txpkgs.remove(hid);
        }
        let gnum = self.txpkgs.len();
        if gnum == 0 {
            self.txpkgs.push(txp);
            return Ok(());
        }
        if gnum >= self.maxsz {
            let tail = self.txpkgs.last().unwrap();
            let lsth = purity_or_fee! { self, txp, <=, tail };
            if lsth {
                return errf!("tx pool is full and your tx fee is too low");
            }
        }
        let mut rxl = 0;
        let mut rxr = gnum;
        if gnum > 10 {
            (rxl, rxr) = scan_group_rng_by_feep(&self.txpkgs, feep, &fee, self.fpmd, (rxl, rxr));
        }
        self.insert_rng(txp, (rxl, rxr))?;
        if self.txpkgs.len() > self.maxsz {
            self.txpkgs.pop();
        }
        Ok(())
    }

    fn insert_rng(&mut self, txp: TxPkg, rng: (usize, usize)) -> Rerr {
        let (rxl, rxr) = rng;
        let mut istx = usize::MAX;
        for i in rxl..rxr {
            let txli = &self.txpkgs[i];
            let bgth = purity_or_fee! { self, txp, >, txli };
            if bgth {
                istx = i;
                break;
            }
        }
        if istx == usize::MAX {
            self.txpkgs.push(txp);
        } else {
            self.txpkgs.insert(istx, txp);
        }
        Ok(())
    }
}
