#[allow(dead_code)]
pub struct MemTxPool {
    lowest_fepr: u64,
    group_size: Vec<usize>,
    groups: Vec<Mutex<TxGroup>>,
}

impl MemTxPool {
    pub fn new(lfepr: u64, gs: Vec<usize>, fpmds: Vec<bool>) -> Self {
        let gslen = gs.len();
        if gslen == 0 || gslen != fpmds.len() {
            never!()
        }
        let mut grps = Vec::with_capacity(gslen);
        for i in 0..gslen {
            let sz = gs[i];
            grps.push(Mutex::new(TxGroup::new(sz, fpmds[i])));
        }
        Self {
            lowest_fepr: lfepr,
            group_size: gs,
            groups: grps,
        }
    }

    fn check_group_id(&self, wgi: usize) -> Rerr {
        if wgi >= self.groups.len() {
            return errf!("tx pool group overflow");
        }
        Ok(())
    }
}

impl TxPool for MemTxPool {
    fn count_at(&self, gi: usize) -> Ret<usize> {
        self.check_group_id(gi)?;
        let count = self.groups[gi].lock().unwrap().txpkgs.len();
        Ok(count)
    }

    fn first_at(&self, gi: usize) -> Ret<Option<TxPkg>> {
        self.check_group_id(gi)?;
        let grp = self.groups[gi].lock().unwrap();
        Ok(grp.txpkgs.first().map(|a| a.clone()))
    }

    fn iter_at(&self, gi: usize, scan: &mut dyn FnMut(&TxPkg) -> bool) -> Rerr {
        self.check_group_id(gi)?;
        let grp = self.groups[gi].lock().unwrap();
        for txp in &grp.txpkgs {
            if false == scan(&txp) {
                break;
            }
        }
        Ok(())
    }

    fn insert_at(&self, gi: usize, txp: TxPkg) -> Rerr {
        if txp.fpur() < self.lowest_fepr {
            return errf!("tx fee purity {} too low to add txpool", txp.fpur());
        }
        self.check_group_id(gi)?;
        let mut grp = self.groups[gi].lock().unwrap();
        grp.insert(txp)?;
        Ok(())
    }

    fn delete_at(&self, gi: usize, hxs: &[Hash]) -> Rerr {
        self.check_group_id(gi)?;
        let mut grp = self.groups[gi].lock().unwrap();
        grp.delete(hxs);
        Ok(())
    }

    fn clear_at(&self, gi: usize) -> Rerr {
        self.check_group_id(gi)?;
        let mut grp = self.groups[gi].lock().unwrap();
        grp.clear();
        Ok(())
    }

    fn find_at(&self, gi: usize, hx: &Hash) -> Option<TxPkg> {
        if self.check_group_id(gi).is_err() {
            return None;
        }
        let grp = self.groups[gi].lock().unwrap();
        match grp.find(hx) {
            Some((_, tx)) => Some(tx.clone()),
            None => None,
        }
    }

    fn retain_at(&self, gi: usize, f: &mut dyn FnMut(&TxPkg) -> bool) -> Rerr {
        self.check_group_id(gi)?;
        self.groups[gi].lock().unwrap().retain(f);
        Ok(())
    }

    fn find(&self, hx: &Hash) -> Option<TxPkg> {
        for gi in 0..self.groups.len() {
            if let Some(tx) = self.find_at(gi, hx) {
                return Some(tx);
            }
        }
        None
    }

    fn insert_by(&self, txp: TxPkg, check_group: &dyn Fn(&TxPkg) -> usize) -> Rerr {
        let group_id = check_group(&txp);
        self.insert_at(group_id, txp)
    }

    fn drain(&self, hxs: &[Hash]) -> Ret<Vec<TxPkg>> {
        let mut txres = vec![];
        let mut hxst = HashSet::from_iter(hxs.to_vec());
        for gi in 0..self.groups.len() {
            let mut grp = self.groups[gi].lock().unwrap();
            let mut res = grp.drain(&mut hxst);
            txres.append(&mut res);
        }
        Ok(txres)
    }

    fn print(&self) -> String {
        let mut shs: Vec<String> = vec![];
        for gi in 0..self.groups.len() {
            if let Ok(gr) = self.groups[gi].try_lock() {
                shs.push(format!("{}({})", gi, gr.txpkgs.len()));
            }
        }
        format!("[TxPool] tx count: {}", shs.join(", "))
    }
}
