
/// Storage rent period, in blocks.
///
/// 300 blocks is treated as ~24h in this chain design note, so:
/// - 100 blocks ~= 8h (work shift)
/// - 3 shifts ~= 1 day
pub const STORAGE_PERIOD: u64     = 100;
/// Maximum rent periods that a storage entry can hold.
///
/// With `STORAGE_PERIOD=100 blocks ~= 8h`, 30,000 periods ~= 10,000 days ~= 27.4 years (365-day year).
pub const STORAGE_PERIOD_MAX: u64 = 30000;
pub const STORAGE_SAVE_MAX: u64   = STORAGE_PERIOD * STORAGE_PERIOD_MAX;

pub const STORAGE_RETAIN_MIN_PERIODS: u64 = 1;
pub const STORAGE_RETAIN_MAX_PERIODS: u64 = 300; // 300 * 8h = 100 days

/// Maximum bytes allowed for a storage key (excluding address prefix).
///
/// This bounds per-op hashing/copy work even when key-hash is not separately metered.
pub const STORAGE_KEY_MAX_BYTES: usize = 256;


// Storage value with a bounded lease + grace window.
//
// NOTE: No backward-compat parsing is provided here; the serialized layout is consensus-critical.
combi_struct!{ ValueSto,
    born: BlockHeight
    expire: BlockHeight
    data: Value
}


impl ValueSto {

    fn new(chei: u64, v: Value) -> Self {
        Self {
            born: BlockHeight::from(chei),
            expire: BlockHeight::from(chei + STORAGE_PERIOD),
            data: v,
        }
    }

    fn lifetime_periods(&self) -> u64 {
        let born = self.born.uint();
        let exp = self.expire.uint();
        let life_blocks = exp.saturating_sub(born);
        let mut periods = life_blocks / STORAGE_PERIOD;
        if periods < 1 {
            periods = 1;
        }
        periods
    }

    fn retain_periods(&self) -> u64 {
        let lp = self.lifetime_periods();
        let mut rp = lp / 3;
        if rp < STORAGE_RETAIN_MIN_PERIODS {
            rp = STORAGE_RETAIN_MIN_PERIODS;
        }
        if rp > STORAGE_RETAIN_MAX_PERIODS {
            rp = STORAGE_RETAIN_MAX_PERIODS;
        }
        rp
    }

    fn max_expire(&self) -> u64 {
        self.born.uint().saturating_add(STORAGE_SAVE_MAX)
    }

    // return (is_expire, is_delete, expire_height)
    //
    // Boundary semantics are strict:
    // - `chei == expire` -> NOT expired yet (still readable/writable; rest=0)
    // - `chei == expire + retain_blocks` -> NOT deleted yet
    fn check(&self, chei: u64) -> (bool, bool, u64) {
        let due = self.expire.uint();
        let isexp = chei > due;
        let retain_blocks = self
            .retain_periods()
            .saturating_mul(STORAGE_PERIOD);
        let isdel = chei > due.saturating_add(retain_blocks);
        (isexp, isdel, due)
    }

    fn update(mut self, chei: u64, v: Value, vbasesz: i64) -> Self {
        let (is_expire, _, due) = self.check(chei);
        // expired entry is treated as a fresh save (new born + minimum lease)
        if is_expire {
            self.data = v;
            self.born = BlockHeight::from(chei);
            self.expire = BlockHeight::from(chei + STORAGE_PERIOD);
            return self;
        }
        // update
        let (new_len, old_len) = (v.can_get_size(), self.data.can_get_size());
        let (new_len, old_len) = match (new_len, old_len) {
            (Ok(new_len), Ok(old_len)) => (new_len as u64, old_len as u64),
            _ => {
                // should not happen for storable values, but keep consensus safe
                self.data = v;
                self.born = BlockHeight::from(chei);
                self.expire = BlockHeight::from(chei + STORAGE_PERIOD);
                return self;
            }
        };
        let vbasesz = vbasesz.max(0) as u64;
        let old_total = old_len.saturating_add(vbasesz);
        let new_total = new_len.saturating_add(vbasesz);
        if old_total == 0 || new_total == 0 {
            // avoid divide-by-zero; treat as fresh save with minimum lease
            self.data = v;
            self.born = BlockHeight::from(chei);
            self.expire = BlockHeight::from(chei + STORAGE_PERIOD);
            return self;
        }
        if new_total != old_total {
            // Maintain "prepaid rent" proportionality:
            // remaining_time_new = remaining_time_old * old_total_size / new_total_size
            let rest = due.saturating_sub(chei) as u128;
            let old_total = old_total as u128;
            let new_total = new_total as u128;
            let mut new_rest = (rest.saturating_mul(old_total) / new_total) as u64;
            if new_rest > STORAGE_SAVE_MAX {
                new_rest = STORAGE_SAVE_MAX;
            }
            let exp = (chei + new_rest).min(self.max_expire());
            self.expire = BlockHeight::from(exp);
        }
        self.data = v;
        self
    }

    // return gas cost
    fn rent(&mut self, gst: &GasExtra, chei: u64, v: Value) -> VmrtRes<i64> {
        let ( _, is_delete, _) = self.check(chei);
        if is_delete {
            return itr_err_fmt!(StorageError, "renewal failed, data invalid")
        }
        if ! v.is_uint() {
            return itr_err_fmt!(StorageError, "period value {:?} is not uint type", v)
        }
        let period = v.to_uint();
        if period < 1 {
            return itr_err_fmt!(StorageError, "period min is 1")
        }
        if period > u16::MAX as u128 {
            return itr_err_fmt!(StorageError, "period value overflow")
        }
        let period = period as u64;
        if period > STORAGE_PERIOD_MAX {
            return itr_err_fmt!(StorageError, "period value max is {} but got {}",
                STORAGE_PERIOD_MAX, period)
        }
        // save (cap by max lease time from born)
        let add_blocks = period
            .checked_mul(STORAGE_PERIOD)
            .ok_or_else(|| ItrErr::new(StorageError, "rent period overflow"))?;
        let exp = self
            .expire
            .uint()
            .checked_add(add_blocks)
            .ok_or_else(|| ItrErr::new(StorageError, "rent expire overflow"))?;
        if exp > self.max_expire() {
            return itr_err_fmt!(
                StorageError,
                "rent overflow, max expire {} but got {}",
                self.max_expire(),
                exp
            );
        }
        self.expire = BlockHeight::from(exp);
        // gas
        let vbasesz = gst.storege_value_base_size;
        let gas = (self.data.can_get_size().unwrap_or(0) as i64 + vbasesz) * period as i64;
        Ok(gas)
    }

}


#[cfg(test)]
mod storage_param_tests {
    use super::*;

    #[derive(Default, Clone)]
    struct StateMem {
        mem: basis::component::MemKV,
    }

    impl State for StateMem {
        fn get(&self, k: Vec<u8>) -> Option<Vec<u8>> {
            match self.mem.get(&k) {
                Some(v) => v.clone(),
                None => None,
            }
        }
        fn set(&mut self, k: Vec<u8>, v: Vec<u8>) {
            self.mem.put(k, v)
        }
        fn del(&mut self, k: Vec<u8>) {
            self.mem.del(k)
        }
    }

    fn test_contract() -> ContractAddress {
        let base = Address::from_readable("1MzNY1oA3kfgYi75zquj3SRUPYztzXHzK9").unwrap();
        ContractAddress::calculate(&base, &Uint4::from(1))
    }

    #[test]
    fn period_is_8_hours_and_max_is_bounded() {
        assert_eq!(STORAGE_PERIOD, 100);
        assert_eq!(STORAGE_PERIOD_MAX, 30000);
        assert_eq!(STORAGE_SAVE_MAX, STORAGE_PERIOD * STORAGE_PERIOD_MAX);
        assert_eq!(STORAGE_RETAIN_MAX_PERIODS, 300);
    }

    #[test]
    fn retain_periods_is_lifetime_div_3_clamped() {
        let mut v = ValueSto::new(0, Value::Nil);
        // lifetime=1 period -> retain clamped to 1
        assert_eq!(v.lifetime_periods(), 1);
        assert_eq!(v.retain_periods(), 1);

        // lifetime=3 periods -> retain=1
        v.born = BlockHeight::from(0);
        v.expire = BlockHeight::from(3 * STORAGE_PERIOD);
        assert_eq!(v.lifetime_periods(), 3);
        assert_eq!(v.retain_periods(), 1);

        // lifetime=30 periods -> retain=10
        v.expire = BlockHeight::from(30 * STORAGE_PERIOD);
        assert_eq!(v.retain_periods(), 10);

        // lifetime >= 1 year (~1095 periods) -> retain capped at 300 (=100 days)
        v.expire = BlockHeight::from(1095 * STORAGE_PERIOD);
        assert_eq!(v.retain_periods(), 300);
    }

    #[test]
    fn rent_cannot_exceed_max_lease() {
        let gst = GasExtra::new(1);
        let mut v = ValueSto::new(0, Value::Nil);
        // rent to exactly the max
        // `ValueSto::new` already includes the minimum 1-period lease, so we can only add
        // `STORAGE_PERIOD_MAX - 1` more periods to reach the maximum.
        let p = Value::U64((STORAGE_PERIOD_MAX - 1) as u64);
        v.rent(&gst, 0, p).unwrap();
        assert_eq!(v.expire.uint(), STORAGE_SAVE_MAX);

        // one more period must fail
        let err = v
            .rent(&gst, 0, Value::U64(1))
            .unwrap_err()
            .to_string();
        assert!(err.contains("rent overflow"));
    }

    #[test]
    fn expire_and_delete_boundary_is_strictly_greater() {
        let v = ValueSto::new(0, Value::Nil);
        // new value: due=100, retain=1 period=100 blocks
        let (is_expire, is_delete, due) = v.check(STORAGE_PERIOD);
        assert_eq!(due, STORAGE_PERIOD);
        assert!(!is_expire);
        assert!(!is_delete);

        let (is_expire, is_delete, _) = v.check(STORAGE_PERIOD + 1);
        assert!(is_expire);
        assert!(!is_delete);

        let (is_expire, is_delete, _) = v.check(STORAGE_PERIOD * 2);
        assert!(is_expire);
        assert!(!is_delete);

        let (_, is_delete, _) = v.check(STORAGE_PERIOD * 2 + 1);
        assert!(is_delete);
    }

    #[test]
    fn srest_returns_zero_at_due_boundary_then_nil_after_expire() {
        let gst = GasExtra::new(1);
        let cadr = test_contract();
        let mut sta = StateMem::default();
        let mut st = crate::VMState::wrap(&mut sta);

        let k = Value::Bytes(vec![1u8; 1]);
        st.ssave(&gst, 0, &cadr, k.clone(), Value::U8(7)).unwrap();

        let rest_due = st.srest(STORAGE_PERIOD, &cadr, &k).unwrap();
        assert_eq!(rest_due, Value::U64(0));

        let rest_expired = st.srest(STORAGE_PERIOD + 1, &cadr, &k).unwrap();
        assert_eq!(rest_expired, Value::Nil);
    }

    #[test]
    fn srent_reclaims_deleted_entry() {
        let gst = GasExtra::new(1);
        let cadr = test_contract();
        let mut sta = StateMem::default();
        let mut st = crate::VMState::wrap(&mut sta);

        let k = Value::Bytes(vec![3u8; 1]);
        st.ssave(&gst, 0, &cadr, k.clone(), Value::U8(9)).unwrap();
        let sk = crate::VMState::skey(&cadr, &k).unwrap();
        assert!(st.ctrtkvdb(&sk).is_some());

        let err = st
            .srent(&gst, STORAGE_PERIOD * 2 + 1, &cadr, k, Value::U8(1))
            .unwrap_err()
            .to_string();
        assert!(err.contains("StorageExpired"));
        assert!(st.ctrtkvdb(&sk).is_none(), "deleted key should be reclaimed");
    }

    #[test]
    fn ssave_rejects_value_larger_than_spacecap() {
        let gst = GasExtra::new(1);
        let cadr = test_contract();
        let mut sta = StateMem::default();
        let mut st = crate::VMState::wrap(&mut sta);

        let max = SpaceCap::new(1).max_value_size;
        let oversized = Value::Bytes(vec![0u8; max + 1]);
        let err = st
            .ssave(&gst, 1, &cadr, Value::Bytes(vec![2u8]), oversized)
            .unwrap_err()
            .to_string();
        assert!(err.contains("StorageValSizeErr"));
    }

    #[test]
    fn ssave_charges_key_create_fee_once_and_treats_expired_as_new() {
        let gst = GasExtra::new(1);
        let cadr = test_contract();
        let mut sta = StateMem::default();
        let mut st = crate::VMState::wrap(&mut sta);

        // force key hashing (addr+key > 32 bytes => sha3)
        let k = Value::Bytes(vec![7u8; 20]);
        let v = Value::Bytes(vec![0u8; 10]);
        let g1 = st.ssave(&gst, 0, &cadr, k.clone(), v.clone()).unwrap();
        let g2 = st.ssave(&gst, 1, &cadr, k.clone(), v.clone()).unwrap();
        assert!(g1 > g2, "first write should include key-create fee");

        // Expire it, then write again: should be treated as (re)create and charge key-create fee.
        {
            let sk = crate::VMState::skey(&cadr, &k).unwrap();
            let mut obj = st.ctrtkvdb(&sk).unwrap();
            obj.expire = BlockHeight::from(0);
            st.ctrtkvdb_set(&sk, &obj);
        }
        let g3 = st.ssave(&gst, 2, &cadr, k, v).unwrap();
        assert!(g3 >= g1, "expired write should be priced as create");
    }
}


/*
* 
*/
inst_state_define!{ VMState,

    201, contract,  ContractAddress  :  ContractSto
    202, ctrtkvdb,  ValueKey         :  ValueSto

}






/*
    state storage
*/
#[allow(dead_code)]
impl VMState<'_> {

    fn skey(cadr: &Address, key: &Value) -> VmrtRes<ValueKey> {
        cadr.check_version().map_ires(StorageError, format!("storage must in dffective address but in {}", cadr))?;
        let k = key.canbe_key()?;
        if k.is_empty() {
            return itr_err_code!(StorageKeyInvalid)
        }
        if k.len() > STORAGE_KEY_MAX_BYTES {
            return itr_err_fmt!(
                StorageKeyInvalid,
                "storage key too long, max {} bytes but got {}",
                STORAGE_KEY_MAX_BYTES,
                k.len()
            );
        }
        let mut k = vec![cadr.to_vec(), k].concat();
        if k.len() > Hash::SIZE {
            k = sys::sha3(k).to_vec();
        }
        Ok(ValueKey::from(k))
    }

    /*
        if not find return Nil  
    */
    fn sread(&mut self, curhei: u64, cadr: &ContractAddress, k: &Value) -> VmrtRes<Option<ValueSto>> {
        let k = Self::skey(cadr, k)?;
        let Some(v) = self.ctrtkvdb(&k) else {
            return Ok(None) // not find
        };
        let (is_expire, is_delete, _) = v.check(curhei);
        if is_delete {
            self.ctrtkvdb_del(&k);
            return Ok(None) // over delete
        }
        if is_expire {
            return Ok(None) // time expire
        }
        Ok(Some(v))
    }


    /*
        if not find return Nil  
    */
    fn sload(&mut self, curhei: u64, cadr: &ContractAddress, k: &Value) -> VmrtRes<Value> {
        let Some(v) = self.sread(curhei, cadr, k)? else {
            return Ok(Value::Nil)
        };
        Ok(v.data)
    }

    /*
        if not find or expire return Nil.
        note: at exact due height rest=0 (not expired yet), expiration starts at due+1.
    */
    fn srest(&mut self, curhei: u64, cadr: &ContractAddress, k: &Value) -> VmrtRes<Value> {
        let Some(v) = self.sread(curhei, cadr, k)? else {
            return Ok(Value::Nil)
        };
        Ok(Value::U64(v.expire.uint() - curhei))
    }

    /*
        read old value 
    */
    fn ssave(&mut self, gst: &GasExtra, curhei: u64, cadr: &ContractAddress, k: Value, v: Value) -> VmrtRes<i64> {
        v.canbe_store()?; // check can store
        let val_len = v.can_get_size()? as usize;
        let max_val = SpaceCap::new(curhei).max_value_size;
        if val_len > max_val {
            return itr_err_fmt!(
                StorageValSizeErr,
                "storage value too large, max {} bytes but got {}",
                max_val,
                val_len
            );
        }

        let k = Self::skey(cadr, &k)?;
        let mut extra_gas = gst.storage_write(val_len);
        let one_period_rent = (val_len as i64) + gst.storege_value_base_size;
        // Key creation fee is charged only when (re)creating a key.
        let key_create_fee = gst.storage_key_cost;

        let mut old_valid = false;
        let old = match self.ctrtkvdb(&k) {
            Some(vold) => {
                let (is_expire, is_delete, _) = vold.check(curhei);
                if is_delete {
                    // over grace window: treat as non-existent (and reclaim space eagerly)
                    self.ctrtkvdb_del(&k);
                    None
                } else if is_expire {
                    // expired is treated as non-existent for SSAVE pricing semantics
                    None
                } else {
                    old_valid = true;
                    Some(vold)
                }
            }
            None => None,
        };

        if !old_valid {
            // (Re)create: charge one period rent + key creation fee.
            extra_gas += one_period_rent + key_create_fee;
        }

        let mut vobj = match old {
            Some(vold) => vold.update(curhei, v, gst.storege_value_base_size),
            None => ValueSto::new(curhei, v),
        };

        // If remaining lease is less than 1 period, SSAVE performs an auto-renew to 1 period.
        // This must not exceed the max lease cap.
        if old_valid {
            let due = vobj.expire.uint();
            let rest = due.saturating_sub(curhei);
            if rest < STORAGE_PERIOD {
                let max_exp = vobj.max_expire();
                let want = curhei.saturating_add(STORAGE_PERIOD);
                if want > max_exp {
                    return itr_err_fmt!(
                        StorageError,
                        "ssave renew overflow, max expire {} but got {}",
                        max_exp,
                        want
                    );
                }
                extra_gas += one_period_rent;
                vobj.expire = BlockHeight::from(want);
            }
        }
        self.ctrtkvdb_set(&k, &vobj);
        Ok(extra_gas)
    }

    // return gas use
    fn srent(&mut self, gst: &GasExtra, curhei: u64, cadr: &ContractAddress, k: Value,  p: Value) -> VmrtRes<i64> {
        let k = Self::skey(cadr, &k)?;
        let Some(mut v) = self.ctrtkvdb(&k) else {
            return itr_err_code!(StorageKeyNotFind)
        };
        let (_, is_delete, _) = v.check(curhei);
        if is_delete {
            self.ctrtkvdb_del(&k);
            return itr_err_fmt!(StorageExpired, "data deleted")
        }
        let gas = v.rent(gst, curhei, p)?;
        self.ctrtkvdb_set(&k, &v);
        Ok(gas)
    }

    fn sdel(&mut self, cadr: &ContractAddress, k: Value) -> VmrtErr {
        let k = Self::skey(cadr, &k)?;
        self.ctrtkvdb_del(&k);
        Ok(())
    }


}
