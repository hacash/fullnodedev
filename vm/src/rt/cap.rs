
#[derive(Debug, Clone, Default)]
pub struct SpaceCap {
    pub loaded_contract: usize, // 20
    pub call_depth: usize, // 32

    pub value_size: usize, // 1280
    pub tuple_length: usize,
    pub compo_length: usize,

    pub stack_slot: usize, // 16*16 = 256
    pub local_slot: usize, // 16*16 = 256

    pub heap_segment: usize, // 64: 256 * 64 = 16kb

    pub global: usize, // 20
    pub memory: usize, // 16

    pub contract_size: usize, // 65535 * 2
    pub function_size: usize, // 65535 / 4
    pub inherit: usize, // 12
    pub library: usize, // 100
    pub reentry_level: u32, // 1, ACTION re-entry level limit

    // pub max_ctl_func: usize, // 200 cache pub max_ctl_libx: usize, // 100 cache pub max_ctl_body: usize, // 50  cache
}

impl SpaceCap {
    pub const DEFAULT_TUPLE_LENGTH: usize = 32;

    pub fn new(_hei: u64) -> SpaceCap {
        const U16M: usize = u16::MAX as usize; // 65535

        SpaceCap {
            loaded_contract: 20,
            call_depth: 32,
            value_size: 1280, // = 32 * 40, diamond name list max bytes: 200*6 = 1200
            tuple_length: Self::DEFAULT_TUPLE_LENGTH,
            compo_length: 128,
            stack_slot: 256,
            local_slot: 256,
            heap_segment: 64,
            global: 20,
            memory: 16,
            contract_size: U16M * 2, // 65535*2
            function_size: U16M / 4, // 65535/4
            inherit: 12,
            library: 100,
            reentry_level: 1, // allow 1 re-entry (2 call layers total)
            // max_ctl_func:   200, max_ctl_libx:   100, max_ctl_body:   50,
        }
    }
}
