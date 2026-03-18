/// Block heights at which VM protocol upgrades activate.
/// Append new heights here for hard forks that change GasTable/GasExtra/SpaceCap.
/// Must be sorted in ascending order.
const UPGRADE_HEIGHTS: &[u64] = &[
    // 200000,  // example: v1 adjustments
];

#[derive(Default)]
pub struct Resoure {
    cfg_height: u64,   // height used to build current config
    next_upgrade: u64, // cached: next upgrade height (skip rebuild if height < this)
    pub gas_table: GasTable,
    pub gas_extra: GasExtra,
    pub space_cap: SpaceCap,
    pub global_map: GKVMap,
    pub memory_map: CtcKVMap,
    pub contracts: HashMap<ContractAddress, Arc<ContractObj>>,
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
            global_map: GKVMap::new(cap.global),
            memory_map: CtcKVMap::new(cap.memory),
            space_cap: cap,
            gas_extra: GasExtra::new(height),
            gas_table: GasTable::new(height),
            ..Default::default()
        }
    }
    pub fn reclaim(&mut self) {
        self.global_map.clear();
        self.memory_map.clear();
        self.contracts.clear();
    }

    pub fn reset(&mut self, height: u64) {
        // Rebuild config when height rolls back below current cfg_height, or crosses next upgrade.
        if height >= self.cfg_height && height < self.next_upgrade {
            return; // same protocol version, skip config rebuild
        }
        // crossed an upgrade boundary — rebuild config
        self.reset_gascap(height);
    }

    fn reset_gascap(&mut self, height: u64) {
        let cap = SpaceCap::new(height);
        self.cfg_height = height;
        self.next_upgrade = Self::next_upgrade_after(height);
        // rebuild
        self.global_map.reset(cap.global);
        self.memory_map.reset(cap.memory);
        self.space_cap = cap;
        self.gas_extra = GasExtra::new(height);
        self.gas_table = GasTable::new(height);
    }

    // Charge one cold contract load with per-load bytes fee.
    pub fn settle_new_contract_load_gas<H: VmHost + ?Sized>(
        &mut self,
        host: &mut H,
        bytes: usize,
    ) -> VmrtErr {
        host.gas_charge(self.gas_extra.load_new_contract + (bytes as i64 / 64))
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
                return h;
            }
        }
        u64::MAX
    }
}






















#[cfg(test)]
mod resource_tests {
    use super::*;
    use sys::XRet;

    struct GasHost {
        remaining: i64,
    }

    impl VmHost for GasHost {
        fn height(&self) -> u64 {
            1
        }

        fn main_entry_bindings(&self) -> FrameBindings {
            FrameBindings::root(Address::default(), Vec::<Address>::new().into())
        }

        fn gas_remaining(&self) -> i64 {
            self.remaining
        }

        fn gas_charge(&mut self, gas: i64) -> VmrtErr {
            if gas < 0 {
                return itr_err_fmt!(GasError, "gas cost invalid: {}", gas);
            }
            self.remaining -= gas;
            if self.remaining < 0 {
                return itr_err_code!(OutOfGas);
            }
            Ok(())
        }

        fn contract_edition(&mut self, _: &ContractAddress) -> Option<ContractEdition> {
            None
        }

        fn contract(&mut self, _: &ContractAddress) -> Option<ContractSto> {
            None
        }

        fn action_call(&mut self, _: u16, _: Vec<u8>) -> XRet<(u32, Vec<u8>)> {
            unreachable!()
        }

        fn log_push(&mut self, _: &Address, _: Vec<Value>) -> VmrtErr {
            unreachable!()
        }

        fn srest(&mut self, _: &Address, _: &Value) -> VmrtRes<Value> {
            unreachable!()
        }

        fn sload(&mut self, _: &Address, _: &Value) -> VmrtRes<Value> {
            unreachable!()
        }

        fn sdel(&mut self, _: &Address, _: Value) -> VmrtErr {
            unreachable!()
        }

        fn ssave(&mut self, _: &GasExtra, _: &Address, _: Value, _: Value) -> VmrtRes<i64> {
            unreachable!()
        }

        fn srent(&mut self, _: &GasExtra, _: &Address, _: Value, _: Value) -> VmrtRes<i64> {
            unreachable!()
        }
    }

    #[test]
    fn settle_new_contract_load_gas_charges_base_plus_bytes_div_64() {
        let mut r = Resoure::create(1);
        let base = r.gas_extra.load_new_contract;
        let mut host = GasHost { remaining: 1000 };
        r.settle_new_contract_load_gas(&mut host, 129).unwrap();
        assert_eq!(host.remaining, 1000 - base - 2);
    }

    #[test]
    fn reset_rebuilds_config_when_height_rolls_back() {
        let mut r = Resoure::create(200);
        assert_eq!(r.cfg_height, 200);
        r.reset(100);
        assert_eq!(r.cfg_height, 100);
    }
}
