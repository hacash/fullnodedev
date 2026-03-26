/// One storage period in blocks.
pub const STORAGE_PERIOD: u64 = 100;
pub const STORAGE_LIVE_MAX_PERIODS: u64 = 30000;
pub const STORAGE_RECV_MAX_PERIODS: u64 = 3000;
pub const STORAGE_LIVE_MAX_BLOCKS: u64 = STORAGE_PERIOD * STORAGE_LIVE_MAX_PERIODS;
pub const STORAGE_RECV_MAX_BLOCKS: u64 = STORAGE_PERIOD * STORAGE_RECV_MAX_PERIODS;
pub const STORAGE_UNIT_BASE_BYTES: u64 = 16;
pub const STORAGE_NEW_SLOT_FEE: i64 = 2048;

/// Maximum bytes allowed for a storage key (excluding address prefix).
pub const STORAGE_KEY_MAX_BYTES: usize = 256;

combi_struct! { ValueSto,
    charge: BlockHeight
    live_credit: Uint8
    recover_credit: Uint8
    data: Value
}

impl ValueSto {
    fn new(chei: u64, data: Value, live_credit: u64, recover_credit: u64) -> Self {
        Self {
            charge: BlockHeight::from(chei),
            live_credit: Uint8::from(live_credit),
            recover_credit: Uint8::from(recover_credit),
            data,
        }
    }

    #[inline(always)]
    fn unit_for(v: &Value) -> VmrtRes<u64> {
        Ok(v.can_get_size()? as u64 + STORAGE_UNIT_BASE_BYTES)
    }

    #[inline(always)]
    fn unit(&self) -> VmrtRes<u64> {
        Self::unit_for(&self.data)
    }

    #[inline(always)]
    fn is_active(&self) -> bool {
        self.live_credit.uint() > 0
    }

    #[inline(always)]
    fn is_recoverable(&self) -> bool {
        self.live_credit.uint() == 0 && self.recover_credit.uint() > 0
    }

    #[inline(always)]
    fn is_absent(&self) -> bool {
        self.live_credit.uint() == 0 && self.recover_credit.uint() == 0
    }

    fn settle(&mut self, curhei: u64) -> VmrtErr {
        let unit = self.unit()?;
        if unit == 0 {
            self.charge = BlockHeight::from(curhei);
            return Ok(());
        }
        let old = self.charge.uint();
        if curhei <= old {
            return Ok(());
        }
        let elapsed = (curhei - old) as u128;
        let unit = unit as u128;
        let mut burn = elapsed.saturating_mul(unit);

        let mut live = self.live_credit.uint() as u128;
        if burn >= live {
            burn -= live;
            live = 0;
        } else {
            live -= burn;
            burn = 0;
        }

        let mut recover = self.recover_credit.uint() as u128;
        if burn >= recover {
            recover = 0;
        } else {
            recover -= burn;
        }

        self.live_credit = Uint8::from(live.min(u64::MAX as u128) as u64);
        self.recover_credit = Uint8::from(recover.min(u64::MAX as u128) as u64);
        self.charge = BlockHeight::from(curhei);
        Ok(())
    }

    #[inline(always)]
    fn live_rest_blocks(&self) -> VmrtRes<u64> {
        let unit = self.unit()?;
        Ok(self.live_credit.uint() / unit)
    }

    #[inline(always)]
    fn recover_rest_blocks(&self) -> VmrtRes<u64> {
        let unit = self.unit()?;
        Ok(self.recover_credit.uint() / unit)
    }
}

#[inline(always)]
fn parse_period(v: Value, max_period: u64) -> VmrtRes<u64> {
    let period = v.extract_u128().map_err(|_| {
        ItrErr::new(
            StorageError,
            &format!("period value {:?} is not uint type", v),
        )
    })?;
    if period < 1 {
        return itr_err_fmt!(StoragePeriodErr, "period min is 1");
    }
    if period > max_period as u128 {
        return itr_err_fmt!(
            StoragePeriodErr,
            "period value max is {} but got {}",
            max_period,
            period
        );
    }
    Ok(period as u64)
}

#[inline(always)]
fn period_credit(unit: u64, period: u64) -> VmrtRes<u64> {
    let blocks = period
        .checked_mul(STORAGE_PERIOD)
        .ok_or_else(|| ItrErr::new(StorageError, "period blocks overflow"))?;
    let credit = (unit as u128)
        .checked_mul(blocks as u128)
        .ok_or_else(|| ItrErr::new(StorageError, "credit overflow"))?;
    if credit > u64::MAX as u128 {
        return itr_err_fmt!(StorageError, "credit overflow");
    }
    Ok(credit as u64)
}

#[inline(always)]
fn u64_to_i64_sat(v: u64) -> i64 {
    v.min(i64::MAX as u64) as i64
}

/* * */
inst_state_define! { VMState,

    201, contract,         ContractAddress  : ContractSto
    202, contract_edition, ContractAddress  : ContractEdition
    205, ctrtkvdb,         ValueKey         : ValueSto

}

/* state storage */
#[allow(dead_code)]
impl VMState<'_> {
    pub fn contract_set_sync_edition(&mut self, addr: &ContractAddress, sto: &ContractSto) {
        self.contract_set(addr, sto);
        self.contract_edition_set(addr, &sto.calc_edition());
    }

    fn skey(cadr: &Address, key: &Value) -> VmrtRes<ValueKey> {
        cadr.check_version().map_ires(
            StorageError,
            format!("storage must be in effective address but got {}", cadr),
        )?;
        let k = key.extract_key_bytes()?;
        if k.is_empty() {
            return itr_err_code!(StorageKeyInvalid);
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

    fn sfetch(&mut self, curhei: u64, sk: &ValueKey) -> VmrtRes<Option<ValueSto>> {
        let Some(mut v) = self.ctrtkvdb(sk) else {
            return Ok(None);
        };
        v.settle(curhei)?;
        if v.is_absent() {
            self.ctrtkvdb_del(sk);
            return Ok(None);
        }
        self.ctrtkvdb_set(sk, &v);
        Ok(Some(v))
    }

    fn sinfo(&mut self, curhei: u64, cadr: &Address, k: &Value) -> VmrtRes<Value> {
        let sk = Self::skey(cadr, k)?;
        let Some(v) = self.sfetch(curhei, &sk)? else {
            return Ok(Value::Nil);
        };
        let live = v.live_rest_blocks()?;
        let recover = v.recover_rest_blocks()?;
        Value::pack_call_args([Value::U64(live), Value::U64(recover)])
    }

    fn sload(&mut self, curhei: u64, cadr: &Address, k: &Value) -> VmrtRes<Value> {
        let sk = Self::skey(cadr, k)?;
        let Some(v) = self.sfetch(curhei, &sk)? else {
            return Ok(Value::Nil);
        };
        if v.is_recoverable() {
            return itr_err_code!(StorageRecoverable);
        }
        Ok(v.data)
    }

    fn snew(
        &mut self,
        _gst: &GasExtra,
        curhei: u64,
        cadr: &Address,
        k: Value,
        v: Value,
        p: Value,
    ) -> VmrtRes<i64> {
        v.check_non_nil_scalar(StorageNilNotAllowed)?;
        let val_len = v.can_get_size()? as usize;
        let max_val = SpaceCap::new(curhei).value_size;
        if val_len > max_val {
            return itr_err_fmt!(
                StorageValSizeErr,
                "storage value too large, max {} bytes but got {}",
                max_val,
                val_len
            );
        }
        let period = parse_period(p, STORAGE_LIVE_MAX_PERIODS)?;
        let sk = Self::skey(cadr, &k)?;
        if self.sfetch(curhei, &sk)?.is_some() {
            return itr_err_code!(StorageKeyExists);
        }
        let unit = val_len as u64 + STORAGE_UNIT_BASE_BYTES;
        let live_credit = period_credit(unit, period)?;
        let vobj = ValueSto::new(curhei, v, live_credit, 0);
        self.ctrtkvdb_set(&sk, &vobj);
        let gas = STORAGE_NEW_SLOT_FEE
            .saturating_add(u64_to_i64_sat(unit).saturating_mul(2))
            .saturating_add(u64_to_i64_sat(unit).saturating_mul(period as i64));
        Ok(gas)
    }

    fn sedit(
        &mut self,
        _gst: &GasExtra,
        curhei: u64,
        cadr: &Address,
        k: Value,
        v: Value,
    ) -> VmrtRes<i64> {
        v.check_non_nil_scalar(StorageNilNotAllowed)?;
        let val_len = v.can_get_size()? as usize;
        let max_val = SpaceCap::new(curhei).value_size;
        if val_len > max_val {
            return itr_err_fmt!(
                StorageValSizeErr,
                "storage value too large, max {} bytes but got {}",
                max_val,
                val_len
            );
        }
        let sk = Self::skey(cadr, &k)?;
        let Some(mut old) = self.sfetch(curhei, &sk)? else {
            return itr_err_code!(StorageKeyNotFind);
        };
        if !old.is_active() {
            return itr_err_code!(StorageRecoverable);
        }
        old.data = v;
        old.charge = BlockHeight::from(curhei);
        self.ctrtkvdb_set(&sk, &old);
        let unit = val_len as u64 + STORAGE_UNIT_BASE_BYTES;
        Ok(u64_to_i64_sat(unit).saturating_mul(2))
    }

    fn srent(
        &mut self,
        _gst: &GasExtra,
        curhei: u64,
        cadr: &Address,
        k: Value,
        p: Value,
    ) -> VmrtRes<i64> {
        let period = parse_period(p, STORAGE_LIVE_MAX_PERIODS)?;
        let sk = Self::skey(cadr, &k)?;
        let Some(mut v) = self.sfetch(curhei, &sk)? else {
            return itr_err_code!(StorageKeyNotFind);
        };
        let unit = v.unit()?;
        let add_credit = period_credit(unit, period)?;
        let add_blocks = period
            .checked_mul(STORAGE_PERIOD)
            .ok_or_else(|| ItrErr::new(StorageError, "rent blocks overflow"))?;
        let cur_blocks = v.live_credit.uint() / unit;
        let next_blocks = cur_blocks
            .checked_add(add_blocks)
            .ok_or_else(|| ItrErr::new(StorageError, "rent overflow"))?;
        if next_blocks > STORAGE_LIVE_MAX_BLOCKS {
            return itr_err_fmt!(
                StoragePeriodErr,
                "live periods max is {}",
                STORAGE_LIVE_MAX_PERIODS
            );
        }
        let next_credit = v
            .live_credit
            .uint()
            .checked_add(add_credit)
            .ok_or_else(|| ItrErr::new(StorageError, "rent credit overflow"))?;
        v.live_credit = Uint8::from(next_credit);
        v.charge = BlockHeight::from(curhei);
        self.ctrtkvdb_set(&sk, &v);
        Ok(u64_to_i64_sat(unit).saturating_mul(period as i64))
    }

    fn srecv(
        &mut self,
        _gst: &GasExtra,
        curhei: u64,
        cadr: &Address,
        k: Value,
        p: Value,
    ) -> VmrtRes<i64> {
        let period = parse_period(p, STORAGE_RECV_MAX_PERIODS)?;
        let sk = Self::skey(cadr, &k)?;
        let Some(mut v) = self.sfetch(curhei, &sk)? else {
            return itr_err_code!(StorageKeyNotFind);
        };
        let unit = v.unit()?;
        let add_credit = period_credit(unit, period)?;
        let add_blocks = period
            .checked_mul(STORAGE_PERIOD)
            .ok_or_else(|| ItrErr::new(StorageError, "recover blocks overflow"))?;
        let cur_blocks = v.recover_credit.uint() / unit;
        let next_blocks = cur_blocks
            .checked_add(add_blocks)
            .ok_or_else(|| ItrErr::new(StorageError, "recover overflow"))?;
        if next_blocks > STORAGE_RECV_MAX_BLOCKS {
            return itr_err_fmt!(
                StoragePeriodErr,
                "recover periods max is {}",
                STORAGE_RECV_MAX_PERIODS
            );
        }
        let next_credit = v
            .recover_credit
            .uint()
            .checked_add(add_credit)
            .ok_or_else(|| ItrErr::new(StorageError, "recover credit overflow"))?;
        v.recover_credit = Uint8::from(next_credit);
        v.charge = BlockHeight::from(curhei);
        self.ctrtkvdb_set(&sk, &v);
        Ok(u64_to_i64_sat(unit)
            .saturating_mul(period as i64)
            .saturating_div(3))
    }

    fn sdel(&mut self, curhei: u64, cadr: &Address, k: Value) -> VmrtRes<i64> {
        let sk = Self::skey(cadr, &k)?;
        let Some(mut v) = self.ctrtkvdb(&sk) else {
            return Ok(0);
        };
        v.settle(curhei)?;
        let refund = u64_to_i64_sat(v.live_credit.uint().saturating_div(STORAGE_PERIOD));
        self.ctrtkvdb_del(&sk);
        Ok(refund)
    }
}
