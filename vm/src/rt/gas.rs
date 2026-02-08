


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
            table: [1; 256]
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
        gst.set(3,  &[BRL, BRS, BRSL, BRSLN, XLG, PUT, CHOISE]);
        // "Medium" cost ops (includes some O(n) stack ops that were previously default-2).
        gst.set(4,  &[
            DUPN, POPN, PICK,
            PBUF, PBUFL,
            MOD, MUL, DIV, XOP, 
            HREAD, HREADU, HREADUL, HSLICE, HGROW,
            ITEMGET, HEAD, BACK, HASKEY, LENGTH,
        ]);
        gst.set(5,  &[POW]);
        gst.set(6,  &[HWRITE, HWRITEX, HWRITEXL, 
            INSERT, REMOVE, CLEAR, APPEND, 
        ]);
        // "Heavy" ops that commonly allocate/copy buffers (previously default-2).
        gst.set(8,  &[
            // bytes operations (often allocate/copy; current implementation clones full buffers)
            CAT, BYTE, CUT, LEFT, RIGHT, LDROP, RDROP,
            MGET, JOIN, REV, 
            NEWLIST, NEWMAP,
            NTCALL
        ]);
        gst.set(12, &[EXTENV, MPUT, CALLTHIS, CALLSELF, CALLSUPER,
            // O(n) compo merge (can touch many items); avoid default-2.
            PACKLIST, PACKMAP, UPLIST, CLONE, MERGE, KEYS, VALUES
        ]);
        gst.set(16, &[EXTFUNC, GGET, CALLCODE]);
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
    heap_read_div: i64,
    heap_write_div: i64,
    log_div: i64,
    storage_read_div: i64,
    storage_write_div: i64,
    compo_byte_div: i64,
    ntcall_div: i64,
    extfunc_div: i64,
    extaction_div: i64,
}

impl GasExtra {
    pub fn new(_hei: u64) -> Self {
        const U16M: i64 = u16::MAX as i64 + 1; // 65536
        Self {
            max_gas_of_tx:     U16M / 8, // 65536/8 = 8192
            local_one_alloc:          5, // 5 * num
            storege_value_base_size: 32,
            load_new_contract:  1 * GSCU as i64, // 32
            main_call_min:      24*2, // 48
            p2sh_call_min:      24*3, // 72
            abst_call_min:      24*4, // 96
            // Space alloc
            memory_key_cost:    20,
            global_key_cost:    32,
            storage_key_cost:   256,
            // Dynamic divisors (byte/N, item/N)
            stack_copy_div:     12,
            heap_read_div:      16,
            heap_write_div:     12,
            log_div:             1,
            storage_read_div:    8,
            storage_write_div:   6,
            compo_byte_div:     20,
            ntcall_div:         16,
            extfunc_div:        16,
            extaction_div:      10,
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
    pub fn ntcall_bytes(&self, len: usize) -> i64 {
        Self::div_bytes(len, self.ntcall_div)
    }

    #[inline(always)]
    pub fn extfunc_bytes(&self, len: usize) -> i64 {
        Self::div_bytes(len, self.extfunc_div)
    }

    #[inline(always)]
    pub fn extaction_bytes(&self, len: usize) -> i64 {
        Self::div_bytes(len, self.extaction_div)
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
