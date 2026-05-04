#[derive(Debug, Clone, Default)]
pub struct SpaceCap {
    pub loaded_contract: usize, // 20
    pub call_depth: usize,      // 32

    pub value_size: usize, // 1280
    pub tuple_length: usize,
    pub compo_length: usize,

    pub storage_period: u64,
    pub storage_live_max_periods: u64,
    pub storage_recv_max_periods: u64,

    pub stack_slot: usize, // 16*16 = 256
    pub local_slot: usize, // 16*16 = 256

    pub heap_segment: usize, // 64: 256 * 64 = 16kb

    pub global: usize,           // 20
    pub memory: usize,           // 16
    pub kv_key_size: usize,      // 128
    pub status_pure_size: usize, // 128

    pub contract_size: usize, // 65535 * 2
    pub function_size: usize, // 65535 / 2
    pub inherit: usize,       // 12
    pub library: usize,       // 100
    pub reentry_level: u32,   // 1, ACTION re-entry level limit

    pub intent_bind_depth: usize, // 10
    /// Max intent instances creatable per execution context (`IntentRuntime` total_created cap).
    pub intent_new: usize,
    /// Max keys per intent map instance (MKVMap entry cap and batch put limits).
    pub intent_key: usize,
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
            storage_period: 100,
            storage_live_max_periods: 30000,
            storage_recv_max_periods: 3000,
            stack_slot: 256,
            local_slot: 256,
            heap_segment: 64,
            global: 20,
            memory: 16,
            kv_key_size: 128,
            status_pure_size: 128,
            contract_size: U16M * 2, // 65535*2
            function_size: U16M / 2, // 65535/2
            inherit: 12,
            library: 100,
            reentry_level: 1, // allow 1 re-entry (2 call layers total)
            intent_bind_depth: 10,
            intent_new: 128,
            intent_key: 32,
        }
    }

    #[inline(always)]
    pub fn storage_live_max_blocks(&self) -> u64 {
        self.storage_period
            .saturating_mul(self.storage_live_max_periods)
    }

    #[inline(always)]
    pub fn storage_recv_max_blocks(&self) -> u64 {
        self.storage_period
            .saturating_mul(self.storage_recv_max_periods)
    }
}

#[cfg(test)]
mod cap_tests {
    use super::*;
    use crate::rt::GasExtra;
    use crate::value::Value;
    use field::Uint4;

    fn max_storage_unit(cap: &SpaceCap, gst: &GasExtra) -> u64 {
        let value = Value::Bytes(vec![0u8; cap.value_size]);
        (value.can_get_size().unwrap() as u64)
            .saturating_add(gst.storege_value_base_size.max(0) as u64)
    }

    #[test]
    fn default_storage_caps_fit_uint4_credit_capacity() {
        let cap = SpaceCap::new(1);
        let gst = GasExtra::new(1);
        let unit = max_storage_unit(&cap, &gst);
        let live_credit = unit.saturating_mul(cap.storage_live_max_blocks());
        let recover_credit = unit.saturating_mul(cap.storage_recv_max_blocks());

        assert!(
            live_credit <= Uint4::MAX as u64,
            "default live storage cap exceeds Uint4 credit capacity"
        );
        assert!(
            recover_credit <= Uint4::MAX as u64,
            "default recover storage cap exceeds Uint4 credit capacity"
        );
    }

    #[test]
    fn increasing_default_like_storage_caps_can_exceed_uint4_capacity() {
        let mut cap = SpaceCap::new(1);
        let gst = GasExtra::new(1);
        cap.storage_live_max_periods = Uint4::MAX as u64;
        cap.storage_recv_max_periods = Uint4::MAX as u64;
        cap.storage_period = 1;
        cap.value_size = u16::MAX as usize - 1;

        let unit = max_storage_unit(&cap, &gst);
        let live_credit = unit.saturating_mul(cap.storage_live_max_blocks());
        let recover_credit = unit.saturating_mul(cap.storage_recv_max_blocks());

        assert!(live_credit > Uint4::MAX as u64);
        assert!(recover_credit > Uint4::MAX as u64);
    }
}
