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
// - 12: MULDIV*, MULSHRUP, DEVSCALED - multiply then divide/round.
// - 14: MULADDDIV, MULSUBDIV, WITHINBPS, LERP - four operands, one divide.
// - 16: WAVG2, MUL3DIV - four operands with extra multiply or sum path.
// - 32: RPOW - high base like storage reads; extra per exponent in execute.
//
// Unrelated opcodes may share a tier when base interpreter cost is similar.
// Reserved bytecode bytes stay at default 1.

pub struct GasTable {
    table: [u8; 256]
}

impl Default for GasTable {
    fn default() -> Self {
        Self {
            table: [1; 256]
        }
    }
}


impl GasTable {

    pub fn new(_hei: u64) -> Self {
        let mut gst = Self { table: [1; 256] };
        gst.set(2, &[AND, OR, EQ, NEQ, LT, GT, LE, GE, NOT]);
        gst.set(3, &[BSHR, BSHL, BXOR, BOR, BAND]);
        // Arithmetic: binary (see module doc ladder)
        gst.set(2, &[ADD, SUB, MAX, MIN, INC, DEC]);
        gst.set(4, &[MUL, DIV, MOD, DIVUP, DIVROUND, SATADD, SATSUB, ABSDIFF]);
        gst.set(6, &[POW, ADDMOD, CLAMP, SQRT, SQRTUP]);
        gst.set(32, &[RPOW]);
        // Arithmetic: triple-operand mul pipeline
        gst.set(8, &[MULADD, MULSUB]);
        gst.set(10, &[MULMOD, MULSHR]);
        gst.set(12, &[MULDIV, MULDIVUP, MULDIVROUND, MULSHRUP, DEVSCALED]);
        // Arithmetic: four-operand
        gst.set(14, &[MULADDDIV, MULSUBDIV, WITHINBPS, LERP]);
        gst.set(16, &[WAVG2, MUL3DIV]);
        // Other
        gst.set(5, &[MGET, GGET, NEWLIST, NEWMAP]);
        gst.set(8, &[PACKLIST, PACKMAP, PACKTUPLE]);
        gst.set(10, &[MPUT, GPUT, CALLSELF, CALLSELFVIEW, CALLSELFPURE]);
        gst.set(12, &[CALLUSEVIEW, CALLUSEPURE]);
        gst.set(16, &[NTFUNC, CALLTHIS, CALLSUPER, CODECALL]);
        gst.set(20, &[LOG1, NTENV, CALLEXTVIEW]);
        gst.set(24, &[LOG2, CALLEXT, CALL]);
        gst.set(28, &[LOG3, ACTENV, SDEL]);
        gst.set(32, &[LOG4, ACTVIEW, SLOAD, SREST]);
        gst.set(48, &[ACTION]);
        gst.set(64, &[SSAVE, SRENT]);
        #[cfg(feature = "calcfunc")]
        gst.set(128, &[CALCCALL]);
        gst
    }

    /*
    pub fn new_bnk(_hei: u64) -> Self {
        use Bytecode::*;
        let mut gst = Self { table : [2; 256] };
        gst.set(1,  &[P0, P1, P2, P3, PU8, PNBUF, PNIL, PTRUE, PFALSE, 
            CU8, CU16, CU32, CU64, CU128, CBUF, CTO, TID, TIS, TNIL, TMAP, TLIST, 
            POP, NOP, NT, END, RET, ABT, ERR, AST, PRT]);
        gst.set(2,  &[]); // all other bytecode
        gst.set(3,  &[BRL, BRS, BRSL, BRSLN, XLG, PUT, PUTX, CHOOSE]);
        gst.set(4,  &[
            DUPN, POPN, ROLL,
            PBUF, PBUFL,
            MOD, MUL, DIV, XOP, 
            HREAD, HREADU, HREADUL, HSLICE, HGROW,
            ITEMGET, HEAD, BACK, HASKEY, LENGTH
        ]);
        gst.set(5,  &[POW, CAT, BYTE, CUT, LEFT, RIGHT, LDROP, RDROP]);
        gst.set(6,  &[NTENV, HWRITE, HWRITEX, HWRITEXL, INSERT, REMOVE, CLEAR, APPEND]);
        gst.set(8,  &[NTFUNC, MGET, JOIN, REV, NEWLIST, NEWMAP]);
        gst.set(10, &[PACKLIST, PACKMAP, PACKTUPLE, TUPLE2LIST, UNPACK, CLONE, MERGE, KEYS, VALUES]);
        gst.set(12, &[ACTENV, MPUT, CALLTHIS, CALLSELF, CALLSUPER, CALLSELFVIEW, CALLSELFPURE,]);
        gst.set(16, &[ACTVIEW, GGET, CODECALL, CALLUSEVIEW, CALLUSEPURE]);
        gst.set(20, &[LOG1]);
        gst.set(24, &[LOG2, GPUT, CALLEXTVIEW]);
        gst.set(28, &[LOG3, SDEL, ACTION]);
        gst.set(32, &[LOG4, SLOAD, SREST, CALLEXT, CALL]); // external-capable call
        gst.set(64, &[SSAVE, SRENT]);
        gst
    }
    */


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


#[derive(Default)]
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
    pub storage_del_min: i64,
    // Dynamic, resource-based gas parameters.
    stack_copy_div: i64,
    stack_write_div: i64,
    stack_cmp_div: i64,
    stack_op_div: i64,
    heap_read_div: i64,
    heap_write_div: i64,
    log_div: i64,
    storage_read_div: i64,
    storage_write_div: i64,
    compile_div: i64,
    compo_byte_div: i64,
    compo_item_read_div: i64,
    compo_item_edit_div: i64,
    compo_item_copy_div: i64,
    ntfunc_div: i64,
    act_div: i64,
}

impl GasExtra {
    pub fn new(_hei: u64) -> Self {
        use protocol::context::*;
        Self {
            compute_limit:   decode_gas_budget(72), // 18009
            resource_limit:  decode_gas_budget(56), // 6100
            storage_limit:   decode_gas_budget(99), // 111911
            // // debug test
            // compute_limit:   0, 
            // resource_limit:  0,
            // storage_limit:   0,
            // Load or alloc 
            one_local_alloc:     5, // 5 * num
            new_contract_load:  32, // base gas for loading a new contract
            main_call_min:    2*24, // 48
            p2sh_call_min:    3*24, // 72
            abst_call_min:    4*24, // 96
            // Space alloc
            memory_key_cost:    20,
            global_key_cost:    32,
            storege_value_base_size: 16,
            storage_key_cost:  256,
            storage_del_min:    16,
            // Dynamic divisors (byte/N, item/N)
            stack_copy_div:     32,
            stack_write_div:    28,
            stack_cmp_div:      24,
            stack_op_div:       20,
            heap_read_div:      16,
            heap_write_div:     12,
            log_div:             1,
            storage_read_div:    1,
            storage_write_div:   1,
            compile_div:         8,
            ntfunc_div:         16,
            act_div:            12,
            // Compo
            compo_byte_div:     40,
            compo_item_read_div: 4,
            compo_item_edit_div: 2,
            compo_item_copy_div: 1,
        }
    }

    #[inline(always)]
    fn div_op(len: usize, div: i64) -> i64 {
        if div <= 0 || len == 0 {
            return 0
        }
        (len as i64 - 1) / div + 1
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
    pub fn ntfunc_bytes(&self, len: usize) -> i64 {
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
        Self::div_op(val_len, self.storage_read_div)
    }

    #[inline(always)]
    pub fn storage_write(&self, val_len: usize) -> i64 {
        Self::div_op(val_len, self.storage_write_div)
    }

    #[inline(always)]
    pub fn compile_bytes(&self, len: usize) -> i64 {
        Self::div_op(len, self.compile_div)
    }

    #[inline(always)]
    pub fn storage_del(&self) -> i64 {
        self.storage_del_min
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
}






/***************************************/




#[cfg(test)]
mod gas_budget_codec_tests {
    use super::*;

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
            assert!(cur > prev, "decode_gas_budget({})={} not > {}", b, cur, prev);
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
                    AND, OR, EQ, NEQ, LT, GT, LE, GE, NOT, ADD, SUB, MAX, MIN, INC, DEC,
                ],
            ),
            (3, vec![BSHR, BSHL, BXOR, BOR, BAND]),
            (4, vec![MUL, DIV, MOD, DIVUP, DIVROUND, SATADD, SATSUB, ABSDIFF]),
            (5, vec![MGET, GGET, NEWLIST, NEWMAP]),
            (6, vec![POW, ADDMOD, CLAMP, SQRT, SQRTUP]),
            (
                8,
                vec![MULADD, MULSUB, PACKLIST, PACKMAP, PACKTUPLE],
            ),
            (
                10,
                vec![
                    MPUT, GPUT, CALLSELF, CALLSELFVIEW, CALLSELFPURE, MULMOD, MULSHR,
                ],
            ),
            (
                12,
                vec![
                    CALLUSEVIEW, CALLUSEPURE, MULDIV, MULDIVUP, MULDIVROUND, MULSHRUP,
                    DEVSCALED,
                ],
            ),
            (14, vec![MULADDDIV, MULSUBDIV, WITHINBPS, LERP]),
            (
                16,
                vec![NTFUNC, CALLTHIS, CALLSUPER, CODECALL, WAVG2, MUL3DIV],
            ),
            (20, vec![LOG1, NTENV, CALLEXTVIEW]),
            (24, vec![LOG2, CALLEXT, CALL]),
            (28, vec![LOG3, ACTENV, SDEL]),
            (32, vec![LOG4, ACTVIEW, SLOAD, SREST, RPOW]),
            (48, vec![ACTION]),
            (64, vec![SSAVE, SRENT]),
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
    fn gas_extra_constants_match_doc() {
        let gst = GasExtra::new(1);
        assert_eq!(gst.main_call_min, 48);
        assert_eq!(gst.p2sh_call_min, 72);
        assert_eq!(gst.abst_call_min, 96);

        assert_eq!(gst.one_local_alloc, 5);
        assert_eq!(gst.memory_key_cost, 20);
        assert_eq!(gst.global_key_cost, 32);
        assert_eq!(gst.new_contract_load, 32);
        assert_eq!(gst.storage_key_cost, 256);
        assert_eq!(gst.storage_del_min, 16);
        assert_eq!(gst.storege_value_base_size, 16);
    }

    #[test]
    fn dynamic_formula_divisors_match_doc() {
        let gst = GasExtra::new(1);

        assert_eq!(gst.stack_copy(0), 0);
        assert_eq!(gst.stack_copy(31), 1);
        assert_eq!(gst.stack_copy(32), 1);
        assert_eq!(gst.stack_copy(64), 2);
        assert_eq!(gst.stack_write(0), 0);
        assert_eq!(gst.stack_write(27), 1);
        assert_eq!(gst.stack_write(28), 1);
        assert_eq!(gst.stack_write(29), 2);
        assert_eq!(gst.stack_write(57), 3);
        assert_eq!(gst.stack_op(0), 0);
        assert_eq!(gst.stack_op(15), 1);
        assert_eq!(gst.stack_op(20), 1);
        assert_eq!(gst.stack_op(32), 2);

        assert_eq!(gst.ntfunc_bytes(0), 0);
        assert_eq!(gst.ntfunc_bytes(15), 1);
        assert_eq!(gst.ntfunc_bytes(16), 1);
        assert_eq!(gst.act_bytes(0), 0);
        assert_eq!(gst.act_bytes(12), 1);
        assert_eq!(gst.act_bytes(13), 2);

        assert_eq!(gst.heap_read(0), 0);
        assert_eq!(gst.heap_read(15), 1);
        assert_eq!(gst.heap_read(16), 1);
        assert_eq!(gst.heap_write(0), 0);
        assert_eq!(gst.heap_write(11), 1);
        assert_eq!(gst.heap_write(12), 1);

        assert_eq!(gst.compo_items_read(0), 0);
        assert_eq!(gst.compo_items_read(3), 1);
        assert_eq!(gst.compo_items_read(4), 1);
        assert_eq!(gst.compo_items_edit(5), 3);
        assert_eq!(gst.compo_items_copy(5), 5);
        assert_eq!(gst.compo_bytes(0), 0);
        assert_eq!(gst.compo_bytes(39), 1);
        assert_eq!(gst.compo_bytes(40), 1);
        assert_eq!(gst.compo_bytes(41), 2);
        assert_eq!(gst.compo_bytes(80), 2);

        assert_eq!(gst.log_bytes(0), 0);
        assert_eq!(gst.log_bytes(37), 37);

        assert_eq!(gst.storage_read(0), 0);
        assert_eq!(gst.storage_read(7), 7);
        assert_eq!(gst.storage_read(8), 8);
        assert_eq!(gst.storage_write(0), 0);
        assert_eq!(gst.storage_write(5), 5);
        assert_eq!(gst.storage_write(6), 6);
        assert_eq!(gst.compile_bytes(0), 0);
        assert_eq!(gst.compile_bytes(15), 2);
        assert_eq!(gst.compile_bytes(16), 2);
        assert_eq!(gst.storage_del(), 16);
    }
}
