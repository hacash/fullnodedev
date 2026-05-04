/***********************************/

// Per-opcode base gas units (`GasTable::gas`). The interpreter may add dynamic metering
// on top (e.g. `RPOW` exponent bits, compare on wide integers).
//
// Arithmetic ladder (see `GasTable::new` grouping):
// - 2: add/sub/min/max/inc/dec - single-step integer ops on stack slots.
// - 4: mul/div/mod, div variants, saturating add/sub, abs diff - widening or division.
// - 6: POW, ADDMOD, CLAMP - extra branches or triple operands without full mul-div path.
// - 8: MULADD, MULSUB - one multiply plus add/sub.
// - 10: MULMOD, MULSHR - multiply then mod or shift.
// - 12: MULDIV*, MULSHRUP, DEVSCALED* - multiply then divide/round.
// - 14: MULADDDIV, MULSUBDIV, WITHINBPS, LERP - four operands, one divide.
// - 16: WAVG2, MUL3DIV - four operands with extra multiply or sum path.
// - 32: RPOW - high base like storage reads; extra per exponent in execute.
//
// Unrelated opcodes may share a tier when base interpreter cost is similar.
// Reserved bytecode bytes stay at default 1.

pub struct GasTable {
    table: [u8; 256],
}

impl Default for GasTable {
    fn default() -> Self {
        Self { table: [1; 256] }
    }
}

impl GasTable {
    pub fn new(_hei: u64) -> Self {
        let mut gst = Self { table: [1; 256] };
        gst.set(2, &[BRL, BRS, BRSL, BRSLN]);
        gst.set(2, &[AND, OR, EQ, NEQ, LT, GT, LE, GE, NOT]);
        gst.set(3, &[BSHR, BSHL, BXOR, BOR, BAND]);
        // Arithmetic: binary (see module doc ladder)
        gst.set(2, &[ADD, SUB, MAX, MIN, INC, DEC]);
        gst.set(4, &[MUL, DIV, MOD, DIVUP, DIVROUND, SATADD, SATSUB, ABSDIFF]);
        gst.set(5, &[SQRT, SQRTUP]);
        gst.set(6, &[POW, ADDMOD, CLAMP]);
        gst.set(32, &[RPOW]);
        // Arithmetic: triple-operand mul pipeline
        gst.set(8, &[MULADD, MULSUB]);
        gst.set(10, &[MULMOD, MULSHR]);
        gst.set(12, &[MULDIV, MULDIVUP, MULDIVROUND, MULSHRUP, DEVSCALED, DEVSCALEDFLOOR]);
        // Arithmetic: four-operand
        gst.set(14, &[MULADDDIV, MULSUBDIV, WITHINBPS, LERP]);
        gst.set(16, &[WAVG2, MUL3DIV]);
        // Other
        gst.set(4, &[INSERT, REMOVE, HEAD, BACK, APPEND]);
        gst.set(5, &[MGET, GGET, NEWLIST, NEWMAP]);
        gst.set(6, &[CLEAR, KEYS, VALUES, TUPLE2LIST, UNPACK]);
        gst.set(8, &[CLONE, MERGE, PACKLIST, PACKMAP, PACKTUPLE]);
        gst.set(10, &[MPUT, GPUT, CALLSELF, CALLSELFVIEW, CALLSELFPURE]);
        gst.set(12, &[MTAKE, CALLUSEVIEW, CALLUSEPURE]);
        gst.set(16, &[NTENV, NTCTL, NTFUNC, CALLTHIS, CALLSUPER, CODECALL]);
        gst.set(20, &[LOG1, CALLEXTVIEW]);
        gst.set(24, &[LOG2, CALLEXT, CALL]);
        gst.set(28, &[LOG3, ACTENV, SDEL]);
        gst.set(32, &[LOG4, ACTVIEW, SLOAD, SSTAT]);
        gst.set(48, &[ACTION]);
        gst.set(64, &[SNEW, SEDIT, SRENT, SRECV, SGET]);
        gst.set(128, &[SPUT]);
        #[cfg(feature = "calcfunc")]
        gst.set(256, &[CALCCALL]);
        gst
    }

    fn set(&mut self, gas: u8, btcds: &[Bytecode]) {
        for cd in btcds {
            let i = *cd as usize;
            self.table[i] = gas;
        }
    }

    #[inline(always)]
    pub fn gas(&self, code: u8) -> i64 {
        self.table[code as usize] as i64
    }
}

/***********************************/

#[derive(Default, Clone)]
pub struct GasExtra {
    pub compute_limit: i64,  // <=0 means disabled
    pub resource_limit: i64, // <=0 means disabled
    pub storage_limit: i64,  // <=0 means disabled
    pub one_local_alloc: i64,
    pub new_contract_load: i64,
    pub main_call_min: i64,
    pub p2sh_call_min: i64,
    pub abst_call_min: i64,
    // Space alloc
    pub memory_key_cost: i64,
    pub global_key_cost: i64,
    pub storege_value_base_size: i64,
    pub storage_key_cost: i64,
    pub storage_edit_mul: i64,
    // Status dynamic gas is independently priced from storage gas.
    pub status_read_byte_mul: i64,
    pub status_write_key_byte_mul: i64,
    pub status_write_value_byte_mul: i64,
    pub container_cmp_header: usize,
    stack_move_item: i64,
    // Dynamic, resource-based gas parameters.
    stack_copy_div: i64,
    stack_write_div: i64,
    stack_cmp_div: i64,
    stack_op_div: i64,
    heap_read_div: i64,
    heap_write_div: i64,
    log_div: i64,
    compile_div: i64,
    contract_div: i64,
    compo_byte_div: i64,
    compo_item_read_div: i64,
    compo_item_edit_div: i64,
    compo_item_copy_div: i64,
    ntfunc_div: i64,
    act_div: i64,
    burn_div: i64,
    rpow_exp_bit_mul: i64,
    rpow_exp_base: i64,
    heap_grow_exp_segments: usize,
    heap_grow_linear_seg: u64,
}

impl GasExtra {
    pub fn new(_hei: u64) -> Self {
        use protocol::context::*;
        Self {
            compute_limit: decode_gas_budget(64),  // 10481
            resource_limit: decode_gas_budget(56), // 6100
            storage_limit: decode_gas_budget(99),  // 111911
            // // debug test
            // compute_limit:   0,
            // resource_limit:  0,
            // storage_limit:   0,
            // Load or alloc
            one_local_alloc: 5,    // 5 * num
            new_contract_load: 32, // base gas for loading a new contract
            main_call_min: 2 * 24, // 48
            p2sh_call_min: 3 * 24, // 72
            abst_call_min: 4 * 24, // 96
            // Space alloc
            memory_key_cost: 20,
            global_key_cost: 32,
            storege_value_base_size: 20,
            storage_key_cost: 1024,
            storage_edit_mul: 4,
            status_read_byte_mul: 8,
            status_write_key_byte_mul: 32,
            status_write_value_byte_mul: 32,
            // other
            container_cmp_header: 12,
            // Dynamic divisors (byte/N, item/N)
            stack_copy_div: 32,
            stack_write_div: 28,
            stack_cmp_div: 24,
            stack_op_div: 20,
            stack_move_item: 1,
            heap_read_div: 16,
            heap_write_div: 12,
            log_div: 1,
            compile_div: 16,
            contract_div: 64,
            ntfunc_div: 16,
            act_div: 12,
            burn_div: 1,
            rpow_exp_bit_mul: 2,
            rpow_exp_base: 1,
            heap_grow_exp_segments: 8,
            heap_grow_linear_seg: 256,
            // Compo
            compo_byte_div: 40,
            compo_item_read_div: 4,
            compo_item_edit_div: 2,
            compo_item_copy_div: 1,
        }
    }

    #[inline(always)]
    fn div_op(len: usize, div: i64) -> i64 {
        if div <= 0 || len == 0 {
            return 0;
        }
        (len as i64 - 1) / div + 1
    }

    #[inline(always)]
    fn linear_bytes(len: usize, mul: i64) -> i64 {
        if mul <= 0 || len == 0 {
            return 0;
        }
        let len = i64::try_from(len).unwrap_or(i64::MAX);
        len.saturating_mul(mul)
    }

    #[inline(always)]
    pub fn stack_copy(&self, len: usize) -> i64 {
        Self::div_op(len, self.stack_copy_div)
    }

    #[inline(always)]
    pub fn stack_write(&self, len: usize) -> i64 {
        Self::div_op(len, self.stack_write_div)
    }

    #[inline(always)]
    pub fn stack_cmp(&self, len: usize) -> i64 {
        Self::div_op(len, self.stack_cmp_div)
    }

    #[inline(always)]
    pub fn stack_op(&self, len: usize) -> i64 {
        Self::div_op(len, self.stack_op_div)
    }

    #[inline(always)]
    pub fn stack_move_items(&self, n: usize) -> i64 {
        if n == 0 {
            return 0;
        }
        self.stack_move_item.saturating_mul(n as i64)
    }

    #[inline(always)]
    pub fn nt_bytes(&self, len: usize) -> i64 {
        Self::div_op(len, self.ntfunc_div)
    }

    #[inline(always)]
    pub fn act_bytes(&self, len: usize) -> i64 {
        Self::div_op(len, self.act_div)
    }

    #[inline(always)]
    pub fn heap_read(&self, len: usize) -> i64 {
        Self::div_op(len, self.heap_read_div)
    }

    #[inline(always)]
    pub fn heap_write(&self, len: usize) -> i64 {
        Self::div_op(len, self.heap_write_div)
    }

    #[inline(always)]
    pub fn log_bytes(&self, total_bytes: usize) -> i64 {
        Self::div_op(total_bytes, self.log_div)
    }

    #[inline(always)]
    pub fn storage_read(&self, val_len: usize) -> i64 {
        (val_len as i64).saturating_add(self.storege_value_base_size.max(0))
    }

    #[inline(always)]
    pub fn storage_write(&self, val_len: usize) -> i64 {
        self.storage_read(val_len).saturating_mul(2)
    }

    #[inline(always)]
    pub fn status_read(&self, val_len: usize) -> i64 {
        Self::linear_bytes(val_len, self.status_read_byte_mul)
    }

    #[inline(always)]
    pub fn status_write(&self, key_len: usize, val_len: usize) -> i64 {
        Self::linear_bytes(key_len, self.status_write_key_byte_mul).saturating_add(
            Self::linear_bytes(val_len, self.status_write_value_byte_mul),
        )
    }

    #[inline(always)]
    pub fn compile_bytes(&self, len: usize) -> i64 {
        Self::div_op(len, self.compile_div)
    }

    #[inline(always)]
    pub fn compo_items_read(&self, n: usize) -> i64 {
        Self::div_op(n, self.compo_item_read_div)
    }

    #[inline(always)]
    pub fn compo_items_edit(&self, n: usize) -> i64 {
        Self::div_op(n, self.compo_item_edit_div)
    }

    #[inline(always)]
    pub fn compo_items_copy(&self, n: usize) -> i64 {
        Self::div_op(n, self.compo_item_copy_div)
    }

    #[inline(always)]
    pub fn compo_bytes(&self, len: usize) -> i64 {
        Self::div_op(len, self.compo_byte_div)
    }

    #[inline(always)]
    pub fn contract_bytes(&self, len: usize) -> i64 {
        Self::div_op(len, self.contract_div)
    }

    #[inline(always)]
    pub fn burn_extra(&self, raw: i64) -> i64 {
        Self::div_op(raw.max(0) as usize, self.burn_div)
    }

    #[inline(always)]
    pub fn rpow_extra(&self, exp_bits: i64) -> i64 {
        exp_bits
            .saturating_mul(self.rpow_exp_bit_mul)
            .saturating_add(self.rpow_exp_base)
    }

    #[inline(always)]
    pub fn heap_grow_gas(&self, oldseg: usize, seg: usize, limit: usize) -> VmrtRes<i64> {
        if oldseg + seg > limit {
            return Err(ItrErr::new(OutOfHeap, "out of heap"));
        }
        // Gas is an abstraction of space usage: the first 8 segments are charged exponentially (2,4,8,16,32,64,128,256), then linear 256 per segment. Price is based on existing heap size so multiple HGROW(1) cannot bypass.
        let mut gas: u64 = 0;
        for s in oldseg..(oldseg + seg) {
            let add = if s < self.heap_grow_exp_segments {
                1u64.checked_shl((s + 1) as u32).unwrap_or(u64::MAX)
            } else {
                self.heap_grow_linear_seg
            };
            gas = gas
                .checked_add(add)
                .ok_or_else(|| ItrErr::new(HeapError, "heap grow gas overflow"))?;
        }
        Ok(gas as i64)
    }
}

/***************************************/

#[cfg(test)]
mod gas_budget_codec_tests {
    use super::*;

    /// Mirrors `GasExtra::div_op` (private) so sample expectations track divisor field changes.
    fn expect_div_op(len: usize, div: i64) -> i64 {
        if div <= 0 || len == 0 {
            0
        } else {
            (len as i64 - 1) / div + 1
        }
    }

    /// Mirrors `GasExtra::linear_bytes` (private) for status-style linear byte costs.
    fn expect_linear_bytes(len: usize, mul: i64) -> i64 {
        if mul <= 0 || len == 0 {
            0
        } else {
            (len as i64).saturating_mul(mul)
        }
    }

    fn encode_gas_budget(gas: i64) -> u8 {
        if gas <= 0 {
            return 0;
        }
        match protocol::context::GAS_BUDGET_LOOKUP_1P07_FROM_138.binary_search(&(gas as u32)) {
            Ok(i) => i as u8,
            Err(i) => {
                if i >= protocol::context::GAS_BUDGET_LOOKUP_1P07_FROM_138.len() {
                    u8::MAX
                } else {
                    i as u8
                }
            }
        }
    }

    #[test]
    fn decode_is_strictly_increasing_for_nonzero_bytes() {
        assert_eq!(protocol::context::decode_gas_budget(0), 0);
        assert_eq!(protocol::context::decode_gas_budget(255), 4_292_817_207);
        let mut prev = protocol::context::decode_gas_budget(0);
        for b in 1u8..=u8::MAX {
            let cur = protocol::context::decode_gas_budget(b);
            assert!(
                cur > prev,
                "decode_gas_budget({})={} not > {}",
                b,
                cur,
                prev
            );
            prev = cur;
        }
    }

    #[test]
    fn encode_decode_roundtrip_on_all_bytes() {
        for b in 0u8..=u8::MAX {
            let gas = protocol::context::decode_gas_budget(b);
            let enc = encode_gas_budget(gas);
            assert_eq!(enc, b, "b={} gas={} enc={}", b, gas, enc);
        }
    }

    #[test]
    fn encode_saturates_to_u8_max_for_out_of_range_budgets() {
        let max = protocol::context::decode_gas_budget(u8::MAX);
        assert_eq!(encode_gas_budget(max + 1), u8::MAX);
        assert_eq!(encode_gas_budget(i64::MAX), u8::MAX);
    }

    #[test]
    fn base_gas_table_matches_impl_and_default_is_1() {
        let gst = GasTable::new(1);
        let mut configured = [false; 256];
        let groups: Vec<(i64, Vec<Bytecode>)> = vec![
            (
                2,
                vec![
                    BRL, BRS, BRSL, BRSLN, AND, OR, EQ, NEQ, LT, GT, LE, GE, NOT, ADD, SUB, MAX,
                    MIN, INC, DEC,
                ],
            ),
            (3, vec![BSHR, BSHL, BXOR, BOR, BAND]),
            (
                4,
                vec![
                    MUL, DIV, MOD, DIVUP, DIVROUND, SATADD, SATSUB, ABSDIFF, INSERT, REMOVE, HEAD,
                    BACK, APPEND,
                ],
            ),
            (5, vec![MGET, GGET, NEWLIST, NEWMAP, SQRT, SQRTUP]),
            (
                6,
                vec![POW, ADDMOD, CLAMP, CLEAR, KEYS, VALUES, TUPLE2LIST, UNPACK],
            ),
            (
                8,
                vec![MULADD, MULSUB, CLONE, MERGE, PACKLIST, PACKMAP, PACKTUPLE],
            ),
            (
                10,
                vec![
                    MPUT,
                    GPUT,
                    CALLSELF,
                    CALLSELFVIEW,
                    CALLSELFPURE,
                    MULMOD,
                    MULSHR,
                ],
            ),
            (
                12,
                vec![
                    MTAKE,
                    CALLUSEVIEW,
                    CALLUSEPURE,
                    MULDIV,
                    MULDIVUP,
                    MULDIVROUND,
                    MULSHRUP,
                    DEVSCALED,
                    DEVSCALEDFLOOR,
                ],
            ),
            (14, vec![MULADDDIV, MULSUBDIV, WITHINBPS, LERP]),
            (
                16,
                vec![
                    NTENV, NTFUNC, NTCTL, CALLTHIS, CALLSUPER, CODECALL, WAVG2, MUL3DIV,
                ],
            ),
            (20, vec![LOG1, CALLEXTVIEW]),
            (24, vec![LOG2, CALLEXT, CALL]),
            (28, vec![LOG3, ACTENV, SDEL]),
            (32, vec![LOG4, ACTVIEW, SLOAD, SSTAT, RPOW]),
            (48, vec![ACTION]),
            (64, vec![SGET, SNEW, SEDIT, SRENT, SRECV]),
            (128, vec![SPUT]),
        ];
        #[cfg(feature = "calcfunc")]
        let mut groups = groups;
        #[cfg(feature = "calcfunc")]
        groups.push((128, vec![CALCCALL]));
        for (gas, items) in &groups {
            for op in items {
                let code = *op as u8;
                configured[code as usize] = true;
                assert_eq!(
                    gst.gas(code),
                    *gas,
                    "base gas mismatch for opcode {:?} (0x{:02x})",
                    op,
                    code
                );
            }
        }
        for code in 0u8..=u8::MAX {
            if !configured[code as usize] {
                assert_eq!(
                    gst.gas(code),
                    1,
                    "opcode 0x{:02x} not listed in impl should default to gas=1",
                    code
                );
            }
        }
    }

    #[test]
    /// Scalar `GasExtra` fields: must match literal assignments in `GasExtra::new`.
    fn gas_extra_field_defaults_match_new() {
        let gst = GasExtra::new(1);
        assert_eq!(gst.main_call_min, 48);
        assert_eq!(gst.p2sh_call_min, 72);
        assert_eq!(gst.abst_call_min, 96);

        assert_eq!(gst.one_local_alloc, 5);
        assert_eq!(gst.memory_key_cost, 20);
        assert_eq!(gst.global_key_cost, 32);
        assert_eq!(gst.new_contract_load, 32);
        assert_eq!(gst.storage_key_cost, 1024);
        assert_eq!(gst.storage_edit_mul, 4);
        assert_eq!(gst.storege_value_base_size, 20);
        assert_eq!(gst.status_read_byte_mul, 8);
        assert_eq!(gst.status_write_key_byte_mul, 32);
        assert_eq!(gst.status_write_value_byte_mul, 32);
    }

    #[test]
    fn dynamic_gas_formulas_match_gas_extra_methods() {
        let gst = GasExtra::new(1);
        let d = expect_div_op;
        let lin = expect_linear_bytes;

        assert_eq!(gst.stack_copy(0), 0);
        assert_eq!(gst.stack_copy(31), d(31, gst.stack_copy_div));
        assert_eq!(gst.stack_copy(32), d(32, gst.stack_copy_div));
        assert_eq!(gst.stack_copy(64), d(64, gst.stack_copy_div));
        assert_eq!(gst.stack_write(0), 0);
        assert_eq!(gst.stack_write(27), d(27, gst.stack_write_div));
        assert_eq!(gst.stack_write(28), d(28, gst.stack_write_div));
        assert_eq!(gst.stack_write(29), d(29, gst.stack_write_div));
        assert_eq!(gst.stack_write(57), d(57, gst.stack_write_div));
        assert_eq!(gst.stack_op(0), 0);
        assert_eq!(gst.stack_op(15), d(15, gst.stack_op_div));
        assert_eq!(gst.stack_op(20), d(20, gst.stack_op_div));
        assert_eq!(gst.stack_op(32), d(32, gst.stack_op_div));

        assert_eq!(gst.nt_bytes(0), 0);
        assert_eq!(gst.nt_bytes(15), d(15, gst.ntfunc_div));
        assert_eq!(gst.nt_bytes(16), d(16, gst.ntfunc_div));
        assert_eq!(gst.act_bytes(0), 0);
        assert_eq!(gst.act_bytes(12), d(12, gst.act_div));
        assert_eq!(gst.act_bytes(13), d(13, gst.act_div));

        assert_eq!(gst.heap_read(0), 0);
        assert_eq!(gst.heap_read(15), d(15, gst.heap_read_div));
        assert_eq!(gst.heap_read(16), d(16, gst.heap_read_div));
        assert_eq!(gst.heap_write(0), 0);
        assert_eq!(gst.heap_write(11), d(11, gst.heap_write_div));
        assert_eq!(gst.heap_write(12), d(12, gst.heap_write_div));

        assert_eq!(gst.compo_items_read(0), 0);
        assert_eq!(gst.compo_items_read(3), d(3, gst.compo_item_read_div));
        assert_eq!(gst.compo_items_read(4), d(4, gst.compo_item_read_div));
        assert_eq!(gst.compo_items_edit(5), d(5, gst.compo_item_edit_div));
        assert_eq!(gst.compo_items_copy(5), d(5, gst.compo_item_copy_div));
        assert_eq!(gst.compo_bytes(0), 0);
        assert_eq!(gst.compo_bytes(39), d(39, gst.compo_byte_div));
        assert_eq!(gst.compo_bytes(40), d(40, gst.compo_byte_div));
        assert_eq!(gst.compo_bytes(41), d(41, gst.compo_byte_div));
        assert_eq!(gst.compo_bytes(80), d(80, gst.compo_byte_div));

        assert_eq!(gst.log_bytes(0), 0);
        assert_eq!(gst.log_bytes(37), d(37, gst.log_div));

        assert_eq!(
            gst.storage_read(0),
            (0i64).saturating_add(gst.storege_value_base_size.max(0))
        );
        assert_eq!(
            gst.storage_read(7),
            (7i64).saturating_add(gst.storege_value_base_size.max(0))
        );
        assert_eq!(
            gst.storage_read(8),
            (8i64).saturating_add(gst.storege_value_base_size.max(0))
        );
        assert_eq!(gst.storage_write(0), gst.storage_read(0).saturating_mul(2));
        assert_eq!(gst.storage_write(5), gst.storage_read(5).saturating_mul(2));
        assert_eq!(gst.storage_write(6), gst.storage_read(6).saturating_mul(2));

        assert_eq!(gst.status_read(0), 0);
        assert_eq!(
            gst.status_read(7),
            lin(7, gst.status_read_byte_mul)
        );
        assert_eq!(
            gst.status_read(8),
            lin(8, gst.status_read_byte_mul)
        );
        assert_eq!(gst.status_write(0, 0), 0);
        assert_eq!(
            gst.status_write(3, 4),
            lin(3, gst.status_write_key_byte_mul)
                .saturating_add(lin(4, gst.status_write_value_byte_mul))
        );
        assert_eq!(
            gst.status_write(3, 3),
            lin(3, gst.status_write_key_byte_mul)
                .saturating_add(lin(3, gst.status_write_value_byte_mul))
        );
        assert_eq!(
            gst.status_write(4, 4),
            lin(4, gst.status_write_key_byte_mul)
                .saturating_add(lin(4, gst.status_write_value_byte_mul))
        );
        assert_eq!(gst.compile_bytes(0), 0);
        assert_eq!(gst.compile_bytes(15), d(15, gst.compile_div));
        assert_eq!(gst.compile_bytes(16), d(16, gst.compile_div));
    }
}
