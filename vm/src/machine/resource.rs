
/// Block heights at which VM protocol upgrades activate.
/// Append new heights here for hard forks that change GasTable/GasExtra/SpaceCap.
/// Must be sorted in ascending order.
const UPGRADE_HEIGHTS: &[u64] = &[
    // 200000,  // example: v1 adjustments
];



#[derive(Default)]
pub struct Resoure {
    cfg_height: u64,        // height used to build current config
    next_upgrade: u64,      // cached: next upgrade height (skip rebuild if height < this)
    pub gas_table: GasTable,
    pub gas_extra: GasExtra,
    pub space_cap: SpaceCap,
    pub global_vals: GKVMap,
    pub memory_vals: CtcKVMap,
    pub contracts: HashMap<ContractAddress, Arc<ContractObj>>,
    pub contract_load_bytes: usize,
    // stack heap
    pub stack_pool: Vec<Stack>,
    pub heap_pool: Vec<Heap>,
}


impl Resoure {

    pub fn create(height: u64) -> Self {
        let cap = SpaceCap::new(height);
        Self {
            cfg_height: height,
            next_upgrade: Self::next_upgrade_after(height),
            global_vals: GKVMap::new(cap.max_global),
            memory_vals: CtcKVMap::new(cap.max_memory),
            space_cap: cap,
            gas_extra: GasExtra::new(height),
            gas_table: GasTable::new(height),
            ..Default::default()
        }
    }

    pub fn reset(&mut self, height: u64) {
        self.global_vals.clear();
        self.memory_vals.clear();
        self.contracts.clear();
        self.contract_load_bytes = 0;
        if height < self.next_upgrade {
            return // same protocol version, skip config rebuild
        }
        // crossed an upgrade boundary — rebuild config
        self.reset_gascap(height);
    }

    fn reset_gascap(&mut self, height: u64) {
        let cap = SpaceCap::new(height);
        self.cfg_height = height;
        self.next_upgrade = Self::next_upgrade_after(height);
        // rebuild
        self.global_vals.reset(cap.max_global);
        self.memory_vals.reset(cap.max_memory);
        self.space_cap = cap;
        self.gas_extra = GasExtra::new(height);
        self.gas_table = GasTable::new(height);
    }

    pub fn stack_allocat(&mut self) -> Stack {
        self.stack_pool.pop().unwrap_or(Stack::default())
    }
    
    pub fn stack_reclaim(&mut self, stk: Stack) {
        self.stack_pool.push(stk);
    }

    pub fn heap_allocat(&mut self) -> Heap {
        self.heap_pool.pop().unwrap_or(Heap::default())
    }
    
    pub fn heap_reclaim(&mut self, heap: Heap) {
        self.heap_pool.push(heap);
    }

    // util

    /// Return the smallest upgrade height that is strictly greater than `height`.
    /// If no future upgrade exists, returns `u64::MAX`.
    fn next_upgrade_after(height: u64) -> u64 {
        for &h in UPGRADE_HEIGHTS {
            if h > height {
                return h
            }
        }
        u64::MAX
    }

}

