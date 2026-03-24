const LOW_BID_PENDING_ERR: &str = "mint.low_bid.pending";

#[derive(Clone)]
struct BiddingRecord {
    usable: bool,
    tarhei: u64,
    time: u64,
    txhx: Hash,
    addr: Address,
    fee: Amount,
}

struct BiddingBook {
    uniq_top: Vec<BiddingRecord>,
}

#[derive(Clone)]
struct LowBidBranch {
    root_fee: Amount,
    blocks: Vec<BlkPkg>,
}

impl LowBidBranch {
    fn create(root: BlkPkg, root_fee: Amount) -> Self {
        Self {
            root_fee,
            blocks: vec![root],
        }
    }

    fn len(&self) -> usize {
        self.blocks.len()
    }

    fn root_hash(&self) -> Hash {
        self.blocks[0].hash()
    }

    fn root_difficulty(&self) -> u32 {
        self.blocks[0].block().difficulty().uint()
    }

    fn tip_hash(&self) -> Hash {
        self.blocks.last().unwrap().hash()
    }

    fn contains(&self, hash: &Hash) -> bool {
        self.blocks.iter().any(|blk| blk.hash() == *hash)
    }

    fn parent_index(&self, prev: &Hash) -> Option<usize> {
        self.blocks.iter().position(|blk| blk.hash() == *prev)
    }

    fn push_child(&mut self, blk: BlkPkg) {
        self.blocks.push(blk);
    }

    fn fork_from_parent(&self, parent_idx: usize, blk: BlkPkg) -> Self {
        let mut blocks = self.blocks[..=parent_idx].to_vec();
        blocks.push(blk);
        Self {
            root_fee: self.root_fee.clone(),
            blocks,
        }
    }
}

struct LowBidGroup {
    dianum: u32,
    height: u64,
    started_at: Instant,
    branches: Vec<LowBidBranch>,
}

impl LowBidGroup {
    fn create(dianum: u32, root: BlkPkg, root_fee: Amount, started_at: Instant) -> Self {
        Self {
            dianum,
            height: root.hein(),
            started_at,
            branches: vec![LowBidBranch::create(root, root_fee)],
        }
    }

    fn branch_num(&self) -> usize {
        self.branches.len()
    }

    fn has_hash(&self, hash: &Hash) -> bool {
        self.branches.iter().any(|branch| branch.contains(hash))
    }

    fn add_root(&mut self, root: BlkPkg, root_fee: Amount, max_branches: usize) -> bool {
        let hash = root.hash();
        if self.has_hash(&hash) {
            return true;
        }
        if self.branches.len() >= max_branches {
            println!(
                "[MintLowBid] group root full height={} diamond={} branches={} max_branches={}",
                self.height,
                self.dianum,
                self.branches.len(),
                max_branches,
            );
            return false;
        }
        self.branches.push(LowBidBranch::create(root, root_fee));
        true
    }

    fn matches_tip(&self, prev: &Hash) -> bool {
        self.branches.iter().any(|branch| branch.tip_hash() == *prev)
    }

    fn try_cache_child(&mut self, blk: BlkPkg, max_len: usize, max_branches: usize) -> bool {
        let hash = blk.hash();
        if self.has_hash(&hash) {
            println!(
                "[MintLowBid] child already cached height={} hash={} group_height={}",
                blk.hein(),
                hash.half(),
                self.height,
            );
            return true;
        }
        let prev = *blk.block().prevhash();
        for idx in 0..self.branches.len() {
            let Some(parent_idx) = self.branches[idx].parent_index(&prev) else {
                continue;
            };
            let next_len = parent_idx + 2;
            if next_len > max_len {
                println!(
                    "[MintLowBid] child over limit group_height={} height={} hash={} prev={} next_len={} max_len={}",
                    self.height,
                    blk.hein(),
                    hash.half(),
                    prev.half(),
                    next_len,
                    max_len,
                );
                return false;
            }
            if parent_idx + 1 == self.branches[idx].len() {
                self.branches[idx].push_child(blk);
            } else {
                if self.branches.len() >= max_branches {
                    println!(
                        "[MintLowBid] group branch full group_height={} height={} hash={} prev={} branches={} max_branches={}",
                        self.height,
                        blk.hein(),
                        hash.half(),
                        prev.half(),
                        self.branches.len(),
                        max_branches,
                    );
                    return false;
                }
                let fork = self.branches[idx].fork_from_parent(parent_idx, blk);
                self.branches.push(fork);
            }
            println!(
                "[MintLowBid] child cached group_height={} diamond={} height={} hash={} prev={} branches={}",
                self.height,
                self.dianum,
                self.branches[idx.min(self.branches.len() - 1)].blocks.last().unwrap().hein(),
                hash.half(),
                prev.half(),
                self.branches.len(),
            );
            return true;
        }
        false
    }

    fn replay_branches(&self) -> Vec<LowBidBranch> {
        let mut branches = self.branches.clone();
        branches.sort_by(|a, b| {
            b.len()
                .cmp(&a.len())
                .then_with(|| b.root_fee.cmp(&a.root_fee))
                .then_with(|| a.root_hash().as_bytes().cmp(b.root_hash().as_bytes()))
        });
        branches
    }
}

struct BiddingProve {
    latest: u32,
    books: HashMap<u32, BiddingBook>,
    low_bid_groups: HashMap<u64, LowBidGroup>,
    replay_allow: HashSet<Hash>,
    block_arrive_time: HashMap<Hash, u64>,
    block_arrive_order: VecDeque<Hash>,
    engine: Option<Weak<dyn Engine>>,
    max_shadow_len: usize,
    max_group_branches: usize,
    loop_started: bool,
    stop: bool,
}

impl BiddingProve {
    fn new(max_shadow_len: usize) -> Self {
        Self {
            latest: 0,
            books: HashMap::new(),
            low_bid_groups: HashMap::new(),
            replay_allow: HashSet::new(),
            block_arrive_time: HashMap::new(),
            block_arrive_order: VecDeque::new(),
            engine: None,
            max_shadow_len,
            max_group_branches: max_shadow_len,
            loop_started: false,
            stop: false,
        }
    }
}

impl BiddingProve {
    const DELAY_SECS: usize = 10;
    const HACD_KEEP: usize = 10;
    const UNIQ_TOP_MAX: usize = 50;
    const LOW_BID_KEEP_SECS: u64 = 2400; // 40 min
    const LOW_BID_LOOP_SECS: u64 = 10;
    const BLOCK_ARRIVE_KEEP: usize = 50;

    fn bind_engine(&mut self, eng: Arc<dyn Engine>) {
        let max_len = eng.config().unstable_block.saturating_mul(10) as usize;
        self.max_shadow_len = max_len.max(1);
        self.max_group_branches = self.max_shadow_len;
        self.engine = Some(Arc::downgrade(&eng));
    }

    fn start_loop(&mut self) -> bool {
        if self.loop_started || self.engine.is_none() {
            return false;
        }
        self.loop_started = true;
        true
    }

    fn record(&mut self, curr_hei: u64, tx: &TxPkg, act: &action::DiamondMint) {
        let dianum = *act.d.number;
        if dianum > self.latest {
            self.latest = dianum;
        }
        let record = BiddingRecord {
            usable: true,
            tarhei: curr_hei / 5 * 5 + 5,
            time: curtimes(),
            txhx: tx.hash(),
            addr: tx.tx().main(),
            fee: tx.tx().fee().clone(),
        };
        let book = self.books.entry(dianum).or_insert_with(|| BiddingBook {
            uniq_top: Vec::new(),
        });
        let mut updated = false;
        for item in book.uniq_top.iter_mut() {
            if item.addr != record.addr {
                continue;
            }
            if record.fee >= item.fee {
                *item = record.clone();
            }
            updated = true;
            break;
        }
        if !updated {
            book.uniq_top.push(record);
        }
        book.uniq_top.sort_by(|a, b| b.fee.cmp(&a.fee).then_with(|| b.time.cmp(&a.time)));
        book.uniq_top.truncate(Self::UNIQ_TOP_MAX);
        self.trim_books();
    }

    fn highest(&self, curhei: u64, dianum: u32, sta: &dyn State, fblkt: u64) -> Amount {
        if fblkt == 0 {
            return Amount::zero();
        }
        let Some(book) = self.books.get(&dianum) else {
            return Amount::zero();
        };
        let coresta = CoreStateRead::wrap(sta);
        let ttx = fblkt.saturating_sub(Self::DELAY_SECS as u64);
        for r in book.uniq_top.iter() {
            let isusa = curhei <= r.tarhei || r.usable;
            if r.time < ttx && isusa {
                let hacbls = coresta.balance(&r.addr).unwrap_or_default();
                if hacbls.hacash >= r.fee {
                    return r.fee.clone();
                }
            }
        }
        Amount::zero()
    }

    fn mark_block_arrival(&mut self, hei: u64, hash: Hash) {
        if hei % 5 != 4 {
            return;
        }
        if self.block_arrive_time.contains_key(&hash) {
            return;
        }
        self.block_arrive_time.insert(hash, curtimes());
        self.block_arrive_order.push_back(hash);
        while self.block_arrive_order.len() > Self::BLOCK_ARRIVE_KEEP {
            let Some(hx) = self.block_arrive_order.pop_front() else {
                break;
            };
            self.block_arrive_time.remove(&hx);
        }
    }

    fn prev_block_arrive_time(&self, prevhx: &Hash) -> u64 {
        self.block_arrive_time
            .get(prevhx)
            .copied()
            .unwrap_or(0)
    }

    fn add_low_bid_root(&mut self, dianum: u32, blk: BlkPkg, root_fee: Amount) -> bool {
        let height = blk.hein();
        let hash = blk.hash();
        match self.low_bid_groups.entry(height) {
            std::collections::hash_map::Entry::Occupied(mut ent) => {
                let group = ent.get_mut();
                if !group.add_root(blk, root_fee.clone(), self.max_group_branches) {
                    return false;
                }
                println!(
                    "[MintLowBid] root grouped height={} hash={} diamond={} branches={} release_in={}s fee={}",
                    height,
                    hash.half(),
                    dianum,
                    group.branch_num(),
                    Self::LOW_BID_KEEP_SECS.saturating_sub(group.started_at.elapsed().as_secs()),
                    root_fee,
                );
                true
            }
            std::collections::hash_map::Entry::Vacant(ent) => {
                let started_at = Instant::now();
                ent.insert(LowBidGroup::create(dianum, blk, root_fee.clone(), started_at));
                println!(
                    "[MintLowBid] root pending height={} hash={} diamond={} branches=1 release_in={}s fee={}",
                    height,
                    hash.half(),
                    dianum,
                    Self::LOW_BID_KEEP_SECS,
                    root_fee,
                );
                true
            }
        }
    }

    fn min_pow_hash_by_prev(&self, prev: &Hash) -> Option<[u8; 32]> {
        for group in self.low_bid_groups.values() {
            for branch in group.branches.iter() {
                if branch.tip_hash() != *prev {
                    continue;
                }
                let max_hash = u32_to_biguint(branch.root_difficulty()).mul(2usize);
                return Some(biguint_to_hash(&max_hash));
            }
        }
        None
    }

    fn cache_low_bid_child(&mut self, blk: BlkPkg) -> bool {
        let prev = *blk.block().prevhash();
        for group in self.low_bid_groups.values_mut() {
            if !group.matches_tip(&prev) {
                continue;
            }
            return group.try_cache_child(blk, self.max_shadow_len, self.max_group_branches);
        }
        false
    }

    fn take_replay_groups(&mut self, root_min: u64, head_max: u64) -> Vec<LowBidGroup> {
        self.low_bid_groups.retain(|_, group| {
            let keep = group.height >= root_min && group.height <= head_max;
            if !keep {
                println!(
                    "[MintLowBid] group dropped height={} diamond={} branches={} root_window=[{}, {}]",
                    group.height,
                    group.dianum,
                    group.branch_num(),
                    root_min,
                    head_max,
                );
            }
            keep
        });
        let mut heights = Vec::new();
        for (height, group) in self.low_bid_groups.iter() {
            if group.started_at.elapsed().as_secs() >= Self::LOW_BID_KEEP_SECS {
                heights.push(*height);
            }
        }
        heights.sort_unstable();
        let mut groups = Vec::with_capacity(heights.len());
        for height in heights {
            if let Some(group) = self.low_bid_groups.remove(&height) {
                groups.push(group);
            }
        }
        groups
    }

    fn allow_replay_chain(&mut self, chain: &[BlkPkg]) {
        for blk in chain.iter() {
            self.replay_allow.insert(blk.hash());
        }
    }

    fn clear_replay_chain(&mut self, hashes: &[Hash]) {
        for hash in hashes.iter() {
            self.replay_allow.remove(hash);
        }
    }

    fn is_replay_allowed(&self, hash: &Hash) -> bool {
        self.replay_allow.contains(hash)
    }

    fn remove_tx(&mut self, dianum: u32, hx: Hash) {
        let Some(book) = self.books.get_mut(&dianum) else {
            return;
        };
        for item in book.uniq_top.iter_mut() {
            if item.txhx == hx {
                item.usable = false;
            }
        }
    }

    fn roll(&mut self, dianum: u32) {
        if dianum > self.latest {
            self.latest = dianum;
        }
        self.trim_books();
    }

    fn trim_books(&mut self) {
        let keep_from = self.latest.saturating_sub(Self::HACD_KEEP as u32 - 1);
        self.books.retain(|num, _| *num >= keep_from);
    }
}

fn low_bid_replay_loop(prove: Arc<Mutex<BiddingProve>>, mut worker: Worker) {
    loop {
        if worker.quit() {
            return;
        }
        std::thread::sleep(Duration::from_secs(BiddingProve::LOW_BID_LOOP_SECS));
        let (engine, groups) = {
            let mut bidding = prove.lock().unwrap();
            if bidding.stop {
                return;
            }
            let Some(engine) = bidding.engine.as_ref().and_then(|it| it.upgrade()) else {
                continue;
            };
            let status = engine.store().status();
            let root_min = status.root_height.uint() + 1;
            let head_max = status.last_height.uint() + 1;
            let groups = bidding.take_replay_groups(root_min, head_max);
            (engine, groups)
        };
        for group in groups.into_iter() {
            replay_low_bid_group(prove.clone(), engine.clone(), group);
        }
    }
}

fn replay_low_bid_group(
    prove: Arc<Mutex<BiddingProve>>,
    engine: Arc<dyn Engine>,
    group: LowBidGroup,
) {
    let branches = group.replay_branches();
    if branches.is_empty() {
        return;
    }
    println!(
        "[MintLowBid] replay begin height={} diamond={} branches={}",
        group.height,
        group.dianum,
        group.branch_num(),
    );
    for branch in branches.into_iter() {
        let chain = branch.blocks;
        let hashes: Vec<Hash> = chain.iter().map(|blk| blk.hash()).collect();
        {
            let mut bidding = prove.lock().unwrap();
            if bidding.stop {
                return;
            }
            bidding.allow_replay_chain(&chain);
        }
        println!(
            "[MintLowBid] replay try height={} diamond={} selected_len={} root_hash={} root_fee={}",
            group.height,
            group.dianum,
            chain.len(),
            hashes[0].half(),
            branch.root_fee,
        );
        let store = engine.store();
        let mut inserted = 0usize;
        let mut success = true;
        for blk in chain.iter() {
            let hash = blk.hash();
            if store.block_data(&hash).is_some() {
                inserted += 1;
                continue;
            }
            match engine.discover(blk.clone()) {
                Ok(_) => {
                    inserted += 1;
                }
                Err(e) => {
                    println!(
                        "[MintLowBid] replay failed height={} hash={} inserted={} err={}",
                        blk.hein(),
                        hash.half(),
                        inserted,
                        e,
                    );
                    success = false;
                    break;
                }
            }
        }
        let mut bidding = prove.lock().unwrap();
        bidding.clear_replay_chain(&hashes);
        println!(
            "[MintLowBid] replay finish height={} diamond={} selected_len={} inserted={} success={}",
            group.height,
            group.dianum,
            chain.len(),
            inserted,
            success,
        );
        if success {
            return;
        }
    }
}
