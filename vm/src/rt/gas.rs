


#[derive(Default)]
pub struct GasUse {
    pub compute: i64,
    pub storage: i64,
}

impl GasUse {
    pub fn total(&self) -> i64 {
        self.compute + self.storage
    }
}


/***********************************/


pub struct GasTable {
    table: [u8; 256]
}

impl Default for GasTable {
    fn default() -> Self {
        Self {
            // Keep Default aligned with `new()` baseline: unspecified opcodes cost 2.
            table: [2; 256]
        }
    }
}


impl GasTable {

    pub fn new(_hei: u64) -> Self {
        use Bytecode::*;
        let mut gst = Self { table : [2; 256] };
        gst.set(1,  &[ PU8, P0, P1, P2, P3, PNBUF, PNIL, PTRUE, PFALSE, 
            CU8, CU16, CU32, CU64, CU128, CBUF, CTO, TID, TIS, TNIL, TMAP, TLIST, 
            POP, NOP, NT, END, RET, ABT, ERR, AST, PRT]);
        gst.set(2,  &[]); // all other bytecode
        gst.set(3,  &[BRL, BRS, BRSL, BRSLN, XLG, PUT, CHOOSE]);
        // "Medium" cost ops (includes some O(n) stack ops that were previously default-2).
        gst.set(4,  &[
            DUPN, POPN, PICK,
            PBUF, PBUFL,
            MOD, MUL, DIV, XOP, 
            HREAD, HREADU, HREADUL, HSLICE, HGROW,
            ITEMGET, HEAD, BACK, HASKEY, LENGTH
        ]);
        gst.set(5,  &[POW]);
        gst.set(6,  &[HWRITE, HWRITEX, HWRITEXL, 
            INSERT, REMOVE, CLEAR, APPEND, 
            NTENV
        ]);
        // "Heavy" ops that commonly allocate/copy buffers (previously default-2).
        gst.set(8,  &[
            // bytes operations (often allocate/copy; current implementation clones full buffers)
            CAT, BYTE, CUT, LEFT, RIGHT, LDROP, RDROP,
            MGET, JOIN, REV, 
            NEWLIST, NEWMAP,
            NTFUNC
        ]);
        gst.set(12, &[EXTENV, MPUT, CALLTHIS, CALLSELF, CALLSUPER,
            // O(n) compo merge (can touch many items); avoid default-2.
            PACKLIST, PACKMAP, UPLIST, CLONE, MERGE, KEYS, VALUES
        ]);
        gst.set(16, &[EXTVIEW, GGET, CALLCODE]);
        gst.set(20, &[LOG1, CALLPURE]);
        gst.set(24, &[LOG2, GPUT, CALLVIEW]);
        gst.set(28, &[LOG3, SDEL, EXTACTION]);
        gst.set(32, &[LOG4, SLOAD, SREST, CALL]); // CALLDYN
        gst.set(64, &[SSAVE, SRENT]);
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


#[derive(Default)]
pub struct GasExtra {
    pub max_gas_of_tx: i64,
    pub gas_rate: i64, // gas burn discount denominator (mainnet=1, L2 sidechain can be e.g. 10 or 32)
    pub local_one_alloc: i64,
    pub storege_value_base_size: i64,
    pub load_new_contract: i64,
    pub main_call_min: i64,
    pub p2sh_call_min: i64,
    pub abst_call_min: i64,
    // Space alloc
    pub memory_key_cost: i64,
    pub global_key_cost: i64,
    pub storage_key_cost: i64,
    // Dynamic, resource-based gas parameters.
    stack_copy_div: i64,
    space_write_div: i64,
    stack_op_div: i64,
    heap_read_div: i64,
    heap_write_div: i64,
    log_div: i64,
    storage_read_div: i64,
    storage_write_div: i64,
    compile_div: i64,
    compo_byte_div: i64,
    ntfunc_div: i64,
    extview_div: i64,
    extaction_div: i64,
    extenv_div: i64,
}

/// Gas budget lookup table for tx `gas_max` byte.
///
/// Generation rule:
/// - `table[i] = floor(138 * 1.07^i)`, `i in 0..=255`.
/// - `table[255] = 4,292,817,207` (fits in `u32`).
/// start from 138 but set it 0
/// Runtime decoding keeps `gas_max=0` as a reserved "no VM gas" value.
/// Therefore `decode_gas_budget(0)=0`, while non-zero bytes use the lookup table.
pub const GAS_BUDGET_LOOKUP_1P07_FROM_138: [u32; 256] = [
    0,   147, 157, 169, 180, 193, 207, 221, 237, 253,
    271, 290, 310, 332, 355, 380, 407, 435, 466, 499,
    534, 571, 611, 654, 699, 748, 801, 857, 917, 981,
    1050, 1124, 1202, 1286, 1376, 1473, 1576, 1686, 1804, 1931,
    2066, 2211, 2365, 2531, 2708, 2898, 3101, 3318, 3550, 3799,
    4065, 4349, 4654, 4979, 5328, 5701, 6100, 6527, 6984, 7473,
    7996, 8556, 9155, 9796, 10481, 11215, 12000, 12840, 13739, 14701,
    15730, 16831, 18009, 19270, 20619, 22062, 23607, 25259, 27027, 28919,
    30944, 33110, 35428, 37908, 40561, 43401, 46439, 49689, 53168, 56889,
    60872, 65133, 69692, 74571, 79791, 85376, 91352, 97747, 104589, 111911,
    119744, 128126, 137095, 146692, 156961, 167948, 179704, 192284, 205743, 220146,
    235556, 252045, 269688, 288566, 308766, 330379, 353506, 378251, 404729, 433060,
    463374, 495811, 530517, 567654, 607389, 649907, 695400, 744078, 796164, 851895,
    911528, 975335, 1043608, 1116661, 1194827, 1278465, 1367958, 1463715, 1566175, 1675807,
    1793114, 1918632, 2052936, 2196642, 2350407, 2514935, 2690980, 2879349, 3080904, 3296567,
    3527327, 3774240, 4038436, 4321127, 4623606, 4947258, 5293566, 5664116, 6060604, 6484847,
    6938786, 7424501, 7944216, 8500311, 9095333, 9732006, 10413247, 11142174, 11922126, 12756675,
    13649643, 14605118, 15627476, 16721399, 17891897, 19144330, 20484433, 21918343, 23452627, 25094311,
    26850913, 28730477, 30741611, 32893523, 35196070, 37659795, 40295981, 43116699, 46134868, 49364309,
    52819811, 56517198, 60473402, 64706540, 69235998, 74082517, 79268294, 84817074, 90754270, 97107068,
    103904563, 111177883, 118960335, 127287558, 136197687, 145731525, 155932732, 166848023, 178527385, 191024302,
    204396003, 218703723, 234012984, 250393893, 267921466, 286675968, 306743286, 328215316, 351190388, 375773715,
    402077876, 430223327, 460338960, 492562687, 527042075, 563935020, 603410472, 645649205, 690844649, 739203775,
    790948039, 846314402, 905556410, 968945359, 1036771534, 1109345541, 1186999729, 1270089710, 1358995990, 1454125710,
    1555914509, 1664828525, 1781366522, 1906062178, 2039486531, 2182250588, 2335008129, 2498458698, 2673350807, 2860485364,
    3060719339, 3274969693, 3504217571, 3749512802, 4011978698, 4292817207,
];

/// Decode gas budget from a 1-byte `gas_max` field using a fixed lookup table.
///
/// - `0`: no gas (non-contract tx / VM disabled)
/// - `1..=255`: lookup at `GAS_BUDGET_LOOKUP_1P07_FROM_138[b]`
#[inline(always)]
pub fn decode_gas_budget(b: u8) -> i64 {
    GAS_BUDGET_LOOKUP_1P07_FROM_138[b as usize] as i64
}

/// Encode an absolute gas budget into the 1-byte `gas_max` lookup field.
///
/// Policy:
/// - `budget <= 0` → `0` (means "no VM gas", used by non-contract tx)
/// - Otherwise, returns the smallest byte `b` such that `decode_gas_budget(b) >= budget`.
/// - Saturates to `255` if `budget` exceeds the encoding range.
pub fn encode_gas_budget(budget: i64) -> u8 {
    if budget <= 0 {
        return 0
    }
    let max = decode_gas_budget(u8::MAX);
    if budget >= max {
        return u8::MAX
    }
    // `decode_gas_budget()` is strictly increasing for b in 1..=255.
    let mut lo: u16 = 1;
    let mut hi: u16 = u8::MAX as u16;
    while lo < hi {
        let mid = (lo + hi) / 2;
        let v = decode_gas_budget(mid as u8);
        if v >= budget {
            hi = mid;
        } else {
            lo = mid + 1;
        }
    }
    lo as u8
}

impl GasExtra {
    pub fn new(_hei: u64) -> Self {
        Self {
            max_gas_of_tx:     8192, // L1 mainnet limit, can increase via hard fork
            gas_rate:          1,    // mainnet: no discount (burn = cost*fee/txsz/gas_rate)
            local_one_alloc:          5, // 5 * num
            storege_value_base_size: 32,
            load_new_contract:  32, // base gas for loading a new contract
            main_call_min:      24*2, // 48
            p2sh_call_min:      24*3, // 72
            abst_call_min:      24*4, // 96
            // Space alloc
            memory_key_cost:    20,
            global_key_cost:    32,
            storage_key_cost:   256,
            // Dynamic divisors (byte/N, item/N)
            stack_copy_div:     24,
            space_write_div:    24,
            stack_op_div:       16,
            heap_read_div:      16,
            heap_write_div:     12,
            log_div:             1,
            storage_read_div:    8,
            storage_write_div:   6,
            compile_div:        12,
            compo_byte_div:     20,
            ntfunc_div:         16,
            extview_div:        16,
            extaction_div:      10,
            extenv_div:         16,
        }
    }

    #[inline(always)]
    fn div_bytes(len: usize, div: i64) -> i64 {
        maybe!(div <= 0, 0, (len as i64) / div)
    }

    #[inline(always)]
    fn div_items(n: usize, div: i64) -> i64 {
        maybe!(div <= 0, 0, (n as i64) / div)
    }

    #[inline(always)]
    pub fn stack_copy(&self, len: usize) -> i64 {
        Self::div_bytes(len, self.stack_copy_div)
    }

    #[inline(always)]
    pub fn space_write(&self, len: usize) -> i64 {
        Self::div_bytes(len, self.space_write_div)
    }

    #[inline(always)]
    pub fn stack_op(&self, len: usize) -> i64 {
        Self::div_bytes(len, self.stack_op_div)
    }

    #[inline(always)]
    pub fn ntfunc_bytes(&self, len: usize) -> i64 {
        Self::div_bytes(len, self.ntfunc_div)
    }

    #[inline(always)]
    pub fn extview_bytes(&self, len: usize) -> i64 {
        Self::div_bytes(len, self.extview_div)
    }

    #[inline(always)]
    pub fn extaction_bytes(&self, len: usize) -> i64 {
        Self::div_bytes(len, self.extaction_div)
    }

    #[inline(always)]
    pub fn extenv_bytes(&self, len: usize) -> i64 {
        Self::div_bytes(len, self.extenv_div)
    }

    #[inline(always)]
    pub fn heap_read(&self, len: usize) -> i64 {
        Self::div_bytes(len, self.heap_read_div)
    }

    #[inline(always)]
    pub fn heap_write(&self, len: usize) -> i64 {
        Self::div_bytes(len, self.heap_write_div)
    }

    #[inline(always)]
    pub fn log_bytes(&self, total_bytes: usize) -> i64 {
        Self::div_bytes(total_bytes, self.log_div)
    }

    #[inline(always)]
    pub fn storage_read(&self, val_len: usize) -> i64 {
        Self::div_bytes(val_len, self.storage_read_div)
    }

    #[inline(always)]
    pub fn storage_write(&self, val_len: usize) -> i64 {
        Self::div_bytes(val_len, self.storage_write_div)
    }

    #[inline(always)]
    pub fn compile_bytes(&self, len: usize) -> i64 {
        Self::div_bytes(len, self.compile_div)
    }

    #[inline(always)]
    pub fn storage_del(&self) -> i64 {
        0
    }

    #[inline(always)]
    pub fn compo_items(&self, n: usize, div: i64) -> i64 {
        Self::div_items(n, div)
    }

    #[inline(always)]
    pub fn compo_bytes(&self, len: usize) -> i64 {
        Self::div_bytes(len, self.compo_byte_div)
    }
}


#[cfg(test)]
mod gas_budget_codec_tests {
    use super::*;

    #[test]
    fn decode_is_strictly_increasing_for_nonzero_bytes() {
        assert_eq!(decode_gas_budget(0), 0);
        assert_eq!(decode_gas_budget(255), 4_292_817_207);
        let mut prev = decode_gas_budget(0);
        for b in 1u8..=u8::MAX {
            let cur = decode_gas_budget(b);
            assert!(cur > prev, "decode_gas_budget({})={} not > {}", b, cur, prev);
            prev = cur;
        }
    }

    #[test]
    fn encode_decode_roundtrip_on_all_bytes() {
        for b in 0u8..=u8::MAX {
            let gas = decode_gas_budget(b);
            let enc = encode_gas_budget(gas);
            assert_eq!(enc, b, "b={} gas={} enc={}", b, gas, enc);
        }
    }

    #[test]
    fn encode_saturates_to_u8_max_for_out_of_range_budgets() {
        let max = decode_gas_budget(u8::MAX);
        assert_eq!(encode_gas_budget(max + 1), u8::MAX);
        assert_eq!(encode_gas_budget(i64::MAX), u8::MAX);
    }

    #[test]
    fn base_gas_table_matches_doc_and_default_is_2() {
        let gst = GasTable::new(1);
        let mut configured = [false; 256];
        let groups: &[(i64, &[Bytecode])] = &[
            (1, &[
                PU8, P0, P1, P2, P3, PNBUF, PNIL, PTRUE, PFALSE,
                CU8, CU16, CU32, CU64, CU128, CBUF, CTO, TID, TIS, TNIL, TMAP, TLIST,
                POP, NOP, NT, END, RET, ABT, ERR, AST, PRT,
            ]),
            (3, &[BRL, BRS, BRSL, BRSLN, XLG, PUT, CHOOSE]),
            (4, &[
                DUPN, POPN, PICK,
                PBUF, PBUFL,
                MOD, MUL, DIV, XOP,
                HREAD, HREADU, HREADUL, HSLICE, HGROW,
                ITEMGET, HEAD, BACK, HASKEY, LENGTH,
            ]),
            (5, &[POW]),
            (6, &[HWRITE, HWRITEX, HWRITEXL, INSERT, REMOVE, CLEAR, APPEND, NTENV]),
            (8, &[CAT, BYTE, CUT, LEFT, RIGHT, LDROP, RDROP, MGET, JOIN, REV, NEWLIST, NEWMAP, NTFUNC]),
            (12, &[EXTENV, MPUT, CALLTHIS, CALLSELF, CALLSUPER, PACKLIST, PACKMAP, UPLIST, CLONE, MERGE, KEYS, VALUES]),
            (16, &[EXTVIEW, GGET, CALLCODE]),
            (20, &[LOG1, CALLPURE]),
            (24, &[LOG2, GPUT, CALLVIEW]),
            (28, &[LOG3, SDEL, EXTACTION]),
            (32, &[LOG4, SLOAD, SREST, CALL]),
            (64, &[SSAVE, SRENT]),
        ];
        for (gas, items) in groups {
            for op in *items {
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
                    2,
                    "opcode 0x{:02x} not listed in doc should default to gas=2",
                    code
                );
            }
        }
    }

    #[test]
    fn gas_extra_constants_match_doc() {
        let gst = GasExtra::new(1);
        assert_eq!(gst.max_gas_of_tx, 8192);
        assert_eq!(gst.main_call_min, 48);
        assert_eq!(gst.p2sh_call_min, 72);
        assert_eq!(gst.abst_call_min, 96);

        assert_eq!(gst.local_one_alloc, 5);
        assert_eq!(gst.memory_key_cost, 20);
        assert_eq!(gst.global_key_cost, 32);
        assert_eq!(gst.load_new_contract, 32);
        assert_eq!(gst.storage_key_cost, 256);
        assert_eq!(gst.storege_value_base_size, 32);
    }

    #[test]
    fn dynamic_formula_divisors_match_doc() {
        let gst = GasExtra::new(1);

        assert_eq!(gst.stack_copy(23), 0);
        assert_eq!(gst.stack_copy(24), 1);
        assert_eq!(gst.stack_copy(49), 2);
        assert_eq!(gst.space_write(23), 0);
        assert_eq!(gst.space_write(24), 1);
        assert_eq!(gst.space_write(49), 2);
        assert_eq!(gst.stack_op(15), 0);
        assert_eq!(gst.stack_op(16), 1);
        assert_eq!(gst.stack_op(32), 2);

        assert_eq!(gst.ntfunc_bytes(15), 0);
        assert_eq!(gst.ntfunc_bytes(16), 1);
        assert_eq!(gst.extview_bytes(31), 1);
        assert_eq!(gst.extview_bytes(32), 2);
        assert_eq!(gst.extenv_bytes(15), 0);
        assert_eq!(gst.extenv_bytes(16), 1);
        assert_eq!(gst.extaction_bytes(9), 0);
        assert_eq!(gst.extaction_bytes(10), 1);

        assert_eq!(gst.heap_read(15), 0);
        assert_eq!(gst.heap_read(16), 1);
        assert_eq!(gst.heap_write(11), 0);
        assert_eq!(gst.heap_write(12), 1);

        assert_eq!(gst.compo_items(3, 4), 0);
        assert_eq!(gst.compo_items(4, 4), 1);
        assert_eq!(gst.compo_items(5, 2), 2);
        assert_eq!(gst.compo_items(5, 1), 5);
        assert_eq!(gst.compo_bytes(19), 0);
        assert_eq!(gst.compo_bytes(20), 1);

        assert_eq!(gst.log_bytes(0), 0);
        assert_eq!(gst.log_bytes(37), 37);

        assert_eq!(gst.storage_read(7), 0);
        assert_eq!(gst.storage_read(8), 1);
        assert_eq!(gst.storage_write(5), 0);
        assert_eq!(gst.storage_write(6), 1);
        assert_eq!(gst.compile_bytes(7), 0);
        assert_eq!(gst.compile_bytes(13), 1);
        assert_eq!(gst.storage_del(), 0);
    }
}
