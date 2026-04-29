combi_struct! { ValueSto,
    charge: BlockHeight
    live_credit: Uint4
    recover_credit: Uint4
    data: Value
}

combi_struct! { StatusKV,
    key: BytesW1
    value: Value
}
combi_list! { StatusKVList, Uint2, StatusKV }

#[derive(Default, Debug, Clone, PartialEq, Eq)]
pub struct StatusSto {
    items: StatusKVList,
}

#[derive(Default, Debug, Clone, PartialEq, Eq)]
struct StatusMap {
    items: BTreeMap<Vec<u8>, Value>,
}

impl StatusMap {
    fn from_storage(sto: &StatusSto) -> VmrtRes<Self> {
        let mut map = BTreeMap::new();
        for item in sto.items.as_list() {
            let value = &item.value;
            if matches!(value, Value::Nil) {
                return itr_err_fmt!(StorageError, "status value cannot be nil in storage");
            }
            value.check_scalar()?;
            let key = item.key.to_vec();
            if key.is_empty() {
                return itr_err_fmt!(StorageError, "status key cannot be empty in storage");
            }
            if map.insert(key, value.clone()).is_some() {
                return itr_err_fmt!(StorageError, "duplicate status key in storage");
            }
        }
        Ok(Self { items: map })
    }

    fn to_storage(&self) -> Ret<StatusSto> {
        let items = self
            .items
            .iter()
            .map(|(key, value)| {
                Ok(StatusKV {
                    key: BytesW1::from(key.clone())?,
                    value: value.clone(),
                })
            })
            .collect::<Ret<Vec<_>>>()?;
        Ok(StatusSto {
            items: StatusKVList::from_list(items)?,
        })
    }

    fn payload_size(&self) -> usize {
        let mut total = 0usize;
        for (k, v) in &self.items {
            total = add_size_saturating(total, k.len());
            if total == usize::MAX {
                break;
            }
            total = add_size_saturating(total, v.val_size());
            if total == usize::MAX {
                break;
            }
        }
        total
    }

    fn validate_key_lengths(&self, key_max: usize, ec: ItrErrCode) -> VmrtErr {
        for key in self.items.keys() {
            if key.len() > key_max {
                return itr_err_fmt!(
                    ec,
                    "status key too long, max {} bytes but got {}",
                    key_max,
                    key.len()
                );
            }
        }
        Ok(())
    }

    fn ensure_save_bounds(&self, cap: &SpaceCap) -> VmrtErr {
        self.validate_key_lengths(cap.kv_key_size, StorageKeyInvalid)?;
        let payload = self.payload_size();
        if payload > cap.status_pure_size {
            return itr_err_fmt!(
                StorageValSizeErr,
                "status payload too large, max {} bytes but got {}",
                cap.status_pure_size,
                payload
            );
        }
        Ok(())
    }

    #[inline(always)]
    fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    #[inline(always)]
    fn get(&self, key: &[u8]) -> Value {
        self.items.get(key).cloned().unwrap_or(Value::Nil)
    }

    fn set_or_remove(&mut self, key: Vec<u8>, value: Value) -> VmrtErr {
        if matches!(value, Value::Nil) {
            self.items.remove(&key);
        } else {
            value.check_scalar()?;
            self.items.insert(key, value);
        }
        Ok(())
    }
}

impl StatusSto {
    fn from_status_map(map: &StatusMap) -> Ret<Self> {
        map.to_storage()
    }

    fn to_status_map(&self) -> VmrtRes<StatusMap> {
        StatusMap::from_storage(self)
    }
}

impl Parse for StatusSto {
    fn parse_from(&mut self, buf: &mut &[u8]) -> Ret<usize> {
        self.items.parse_from(buf)
    }
}

impl Serialize for StatusSto {
    fn serialize_to(&self, out: &mut Vec<u8>) {
        self.items.serialize_to(out)
    }

    fn size(&self) -> usize {
        self.items.size()
    }
}

impl_field_only_new! { StatusSto }

impl ToJSON for StatusSto {
    fn to_json_fmt(&self, fmt: &JSONFormater) -> String {
        self.items.to_json_fmt(fmt)
    }
}

impl FromJSON for StatusSto {
    fn from_json(&mut self, json: &str) -> Ret<()> {
        self.items.from_json(json)
    }
}

impl ValueSto {
    fn credit_u32(v: u64, tip: &str) -> VmrtRes<u32> {
        u32::try_from(v).map_err(|_| ItrErr::new(StorageError, tip))
    }

    fn new(chei: u64, data: Value, live_credit: u64, recover_credit: u64) -> VmrtRes<Self> {
        Ok(Self {
            charge: BlockHeight::from(chei),
            live_credit: Uint4::from(Self::credit_u32(live_credit, "live credit overflow")?),
            recover_credit: Uint4::from(Self::credit_u32(
                recover_credit,
                "recover credit overflow",
            )?),
            data,
        })
    }

    #[inline(always)]
    fn unit_for(gst: &GasExtra, v: &Value) -> VmrtRes<u64> {
        Ok((v.can_get_size()? as u64).saturating_add(gst.storege_value_base_size.max(0) as u64))
    }

    #[inline(always)]
    fn unit(&self, gst: &GasExtra) -> VmrtRes<u64> {
        Self::unit_for(gst, &self.data)
    }

    #[inline(always)]
    fn is_active(&self) -> bool {
        self.live_credit.uint() > 0
    }

    #[inline(always)]
    fn is_recoverable(&self) -> bool {
        // Recoverable means the entry is still kept on-chain, but it is not active:
        // it cannot be read or edited directly, yet it may still be renewed or deleted.
        self.live_credit.uint() == 0 && self.recover_credit.uint() > 0
    }

    #[inline(always)]
    fn is_absent(&self) -> bool {
        self.live_credit.uint() == 0 && self.recover_credit.uint() == 0
    }

    fn settle(&mut self, curhei: u64, gst: &GasExtra) -> VmrtErr {
        let unit = self.unit(gst)?;
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

        self.live_credit = Uint4::from(Self::credit_u32(
            live.min(u64::MAX as u128) as u64,
            "live credit overflow",
        )?);
        self.recover_credit = Uint4::from(Self::credit_u32(
            recover.min(u64::MAX as u128) as u64,
            "recover credit overflow",
        )?);
        self.charge = BlockHeight::from(curhei);
        Ok(())
    }

    #[inline(always)]
    fn live_rest_blocks(&self, gst: &GasExtra) -> VmrtRes<u64> {
        let unit = self.unit(gst)?;
        rest_blocks(self.live_credit.uint() as u64, unit)
    }

    #[inline(always)]
    fn recover_rest_blocks(&self, gst: &GasExtra) -> VmrtRes<u64> {
        let unit = self.unit(gst)?;
        rest_blocks(self.recover_credit.uint() as u64, unit)
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
fn period_credit(unit: u64, period: u64, storage_period: u64) -> VmrtRes<u64> {
    let blocks = period
        .checked_mul(storage_period)
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

#[inline(always)]
fn rest_blocks(credit: u64, unit: u64) -> VmrtRes<u64> {
    if unit == 0 {
        return itr_err_fmt!(StorageError, "storage unit cannot be zero");
    }
    if credit == 0 {
        Ok(0)
    } else {
        Ok(credit.saturating_sub(1) / unit + 1)
    }
}

#[inline(always)]
fn refund_for_live_credit(credit: u64, storage_period: u64) -> i64 {
    u64_to_i64_sat(credit.saturating_div(storage_period))
}

#[inline(always)]
fn credit_cap_for_blocks(unit: u64, blocks: u64, tip: &str) -> VmrtRes<u64> {
    let credit = (unit as u128)
        .checked_mul(blocks as u128)
        .ok_or_else(|| ItrErr::new(StorageError, tip))?;
    if credit > u64::MAX as u128 {
        return itr_err_fmt!(StorageError, "{}", tip);
    }
    Ok(credit as u64)
}

#[inline(always)]
fn clamp_credit_to_cap(credit: u64, cap: u64) -> (u64, u64) {
    let next = credit.min(cap);
    let trimmed = credit.saturating_sub(next);
    (next, trimmed)
}

/* * */
inst_state_define! { VMState,

    201, contract,         ContractAddress  : ContractSto
    202, contract_edition, ContractAddress  : ContractEdition
    205, ctrtkvdb,         ValueKey         : ValueSto
    206, ctrtstatus,       ContractAddress  : StatusSto

}

/* state storage */
#[allow(dead_code)]
impl VMState<'_> {
    pub fn contract_set_sync_edition(&mut self, addr: &ContractAddress, sto: &ContractSto) {
        self.contract_set(addr, sto);
        self.contract_edition_set(addr, &sto.calc_edition());
    }

    fn status_key_max(cap: &SpaceCap) -> usize {
        cap.kv_key_size
    }

    fn status_contract_addr(cadr: &Address) -> VmrtRes<ContractAddress> {
        ContractAddress::from_addr(*cadr).map_ires(
            StorageError,
            format!(
                "status storage must be in contract address but got {}",
                cadr
            ),
        )
    }

    fn status_key_bytes(cap: &SpaceCap, key: &Value) -> VmrtRes<Vec<u8>> {
        let key = key.extract_key_bytes_with_error_code(StorageKeyInvalid)?;
        let key_max = Self::status_key_max(cap);
        if key.len() > key_max {
            return itr_err_fmt!(
                StorageKeyInvalid,
                "status key too long, max {} bytes but got {}",
                key_max,
                key.len()
            );
        }
        Ok(key)
    }

    fn status_load_by_contract(
        &self,
        cap: &SpaceCap,
        caddr: &ContractAddress,
    ) -> VmrtRes<StatusMap> {
        match self.ctrtstatus(caddr) {
            Some(sto) => {
                let status = sto.to_status_map()?;
                status.validate_key_lengths(Self::status_key_max(cap), StorageError)?;
                Ok(status)
            }
            None => Ok(StatusMap::default()),
        }
    }

    fn status_load(&self, cap: &SpaceCap, cadr: &Address) -> VmrtRes<StatusMap> {
        let caddr = Self::status_contract_addr(cadr)?;
        self.status_load_by_contract(cap, &caddr)
    }

    fn status_save_by_contract(
        &mut self,
        cap: &SpaceCap,
        caddr: &ContractAddress,
        status: &StatusMap,
    ) -> VmrtRes<()> {
        if status.is_empty() {
            self.ctrtstatus_del(caddr);
            return Ok(());
        }
        status.ensure_save_bounds(cap)?;
        let sto = StatusSto::from_status_map(status)
            .map_ires(StorageError, "serialize status object failed".to_owned())?;
        self.ctrtstatus_set(caddr, &sto);
        Ok(())
    }

    fn status_save(&mut self, cap: &SpaceCap, cadr: &Address, status: &StatusMap) -> VmrtRes<()> {
        let caddr = Self::status_contract_addr(cadr)?;
        self.status_save_by_contract(cap, &caddr, status)
    }

    pub(crate) fn status_get_gas(gst: &GasExtra, value: &Value) -> i64 {
        maybe!(matches!(value, Value::Nil), 0i64, gst.status_read(value.val_size()))
    }

    pub(crate) fn status_put_gas(gst: &GasExtra, key: &Value, value: &Value) -> VmrtRes<i64> {
        let klen = key
            .extract_key_bytes_with_error_code(StorageKeyInvalid)?
            .len();
        let vlen = maybe!(matches!(value, Value::Nil), 0usize, value.val_size());
        Ok(gst.status_write(klen, vlen))
    }

    fn sget(&self, cap: &SpaceCap, cadr: &Address, k: &Value) -> VmrtRes<Value> {
        let key = Self::status_key_bytes(cap, k)?;
        Ok(self.status_load(cap, cadr)?.get(&key))
    }

    fn sput(&mut self, cap: &SpaceCap, cadr: &Address, k: Value, v: Value) -> VmrtErr {
        let caddr = Self::status_contract_addr(cadr)?;
        let key = Self::status_key_bytes(cap, &k)?;
        let mut status = self.status_load_by_contract(cap, &caddr)?;
        status.set_or_remove(key, v)?;
        self.status_save_by_contract(cap, &caddr, &status)
    }

    fn skey(cadr: &Address, key: &Value, key_max: usize) -> VmrtRes<ValueKey> {
        cadr.check_version().map_ires(
            StorageError,
            format!("storage must be in effective address but got {}", cadr),
        )?;
        let k = key.extract_key_bytes_with_error_code(StorageKeyInvalid)?;
        if k.len() > key_max {
            return itr_err_fmt!(
                StorageKeyInvalid,
                "storage key too long, max {} bytes but got {}",
                key_max,
                k.len()
            );
        }
        let mut k = vec![cadr.to_vec(), k].concat();
        if k.len() > Hash::SIZE {
            k = sys::sha3(k).to_vec();
        }
        Ok(ValueKey::from(k))
    }

    fn sfetch(&mut self, curhei: u64, gst: &GasExtra, sk: &ValueKey) -> VmrtRes<Option<ValueSto>> {
        let Some(mut v) = self.ctrtkvdb(sk) else {
            return Ok(None);
        };
        v.settle(curhei, gst)?;
        if v.is_absent() {
            self.ctrtkvdb_del(sk);
            return Ok(None);
        }
        self.ctrtkvdb_set(sk, &v);
        Ok(Some(v))
    }

    fn sstat(
        &mut self,
        gst: &GasExtra,
        cap: &SpaceCap,
        curhei: u64,
        cadr: &Address,
        k: &Value,
    ) -> VmrtRes<Value> {
        let sk = Self::skey(cadr, k, cap.value_size)?;
        let Some(v) = self.sfetch(curhei, gst, &sk)? else {
            return Ok(Value::Nil);
        };
        let live = v.live_rest_blocks(gst)?;
        let recover = v.recover_rest_blocks(gst)?;
        Value::pack_tuple([Value::U64(live), Value::U64(recover)])
    }

    fn sload(
        &mut self,
        gst: &GasExtra,
        cap: &SpaceCap,
        curhei: u64,
        cadr: &Address,
        k: &Value,
    ) -> VmrtRes<Value> {
        let sk = Self::skey(cadr, k, cap.value_size)?;
        let Some(v) = self.sfetch(curhei, gst, &sk)? else {
            return Ok(Value::Nil);
        };
        if v.is_recoverable() {
            return itr_err_code!(StorageNotActive);
        }
        Ok(v.data)
    }

    fn snew(
        &mut self,
        gst: &GasExtra,
        cap: &SpaceCap,
        curhei: u64,
        cadr: &Address,
        k: Value,
        v: Value,
        p: Value,
    ) -> VmrtRes<i64> {
        v.check_non_nil_scalar(StorageNilNotAllowed)?;
        let val_len = v.can_get_size()? as usize;
        let max_val = cap.value_size;
        if val_len > max_val {
            return itr_err_fmt!(
                StorageValSizeErr,
                "storage value too large, max {} bytes but got {}",
                max_val,
                val_len
            );
        }
        let period = parse_period(p, cap.storage_live_max_periods)?;
        let sk = Self::skey(cadr, &k, cap.value_size)?;
        if self.sfetch(curhei, gst, &sk)?.is_some() {
            return itr_err_code!(StorageKeyExists);
        }
        let unit = ValueSto::unit_for(gst, &v)?;
        let live_credit = period_credit(unit, period, cap.storage_period)?;
        let vobj = ValueSto::new(curhei, v, live_credit, 0)?;
        self.ctrtkvdb_set(&sk, &vobj);
        let gas = gst
            .storage_key_cost
            .saturating_add(u64_to_i64_sat(unit).saturating_mul(period as i64));
        Ok(gas)
    }

    fn sedit(
        &mut self,
        gst: &GasExtra,
        cap: &SpaceCap,
        curhei: u64,
        cadr: &Address,
        k: Value,
        v: Value,
    ) -> VmrtRes<(i64, i64)> {
        v.check_non_nil_scalar(StorageNilNotAllowed)?;
        let val_len = v.can_get_size()? as usize;
        let max_val = cap.value_size;
        if val_len > max_val {
            return itr_err_fmt!(
                StorageValSizeErr,
                "storage value too large, max {} bytes but got {}",
                max_val,
                val_len
            );
        }
        let sk = Self::skey(cadr, &k, cap.value_size)?;
        let Some(mut old) = self.sfetch(curhei, gst, &sk)? else {
            return itr_err_code!(StorageKeyNotFind);
        };
        if !old.is_active() {
            return itr_err_code!(StorageNotActive);
        }
        old.data = v;
        old.charge = BlockHeight::from(curhei);
        let unit = ValueSto::unit_for(gst, &old.data)?;
        let live_cap = credit_cap_for_blocks(
            unit,
            cap.storage_live_max_blocks(),
            "live credit cap overflow",
        )?;
        let recover_cap = credit_cap_for_blocks(
            unit,
            cap.storage_recv_max_blocks(),
            "recover credit cap overflow",
        )?;
        let (live_credit, trimmed_live) =
            clamp_credit_to_cap(old.live_credit.uint() as u64, live_cap);
        let (recover_credit, _) =
            clamp_credit_to_cap(old.recover_credit.uint() as u64, recover_cap);
        old.live_credit = Uint4::from(ValueSto::credit_u32(
            live_credit,
            "edit live credit overflow",
        )?);
        old.recover_credit = Uint4::from(ValueSto::credit_u32(
            recover_credit,
            "edit recover credit overflow",
        )?);
        self.ctrtkvdb_set(&sk, &old);
        let fee = u64_to_i64_sat(unit).saturating_mul(gst.storage_edit_mul);
        let rebate = refund_for_live_credit(trimmed_live, cap.storage_period);
        Ok((fee, rebate))
    }

    fn srent(
        &mut self,
        gst: &GasExtra,
        cap: &SpaceCap,
        curhei: u64,
        cadr: &Address,
        k: Value,
        p: Value,
    ) -> VmrtRes<i64> {
        let period = parse_period(p, cap.storage_live_max_periods)?;
        let sk = Self::skey(cadr, &k, cap.value_size)?;
        let Some(mut v) = self.sfetch(curhei, gst, &sk)? else {
            return itr_err_code!(StorageKeyNotFind);
        };
        let unit = v.unit(gst)?;
        let add_credit = period_credit(unit, period, cap.storage_period)?;
        let add_blocks = period
            .checked_mul(cap.storage_period)
            .ok_or_else(|| ItrErr::new(StorageError, "rent blocks overflow"))?;
        let cur_blocks = rest_blocks(v.live_credit.uint() as u64, unit)?;
        let next_blocks = cur_blocks
            .checked_add(add_blocks)
            .ok_or_else(|| ItrErr::new(StorageError, "rent overflow"))?;
        if next_blocks > cap.storage_live_max_blocks() {
            return itr_err_fmt!(
                StoragePeriodErr,
                "live block budget exceeded, max {} blocks",
                cap.storage_live_max_blocks()
            );
        }
        let next_credit = (v.live_credit.uint() as u64)
            .checked_add(add_credit)
            .ok_or_else(|| ItrErr::new(StorageError, "rent credit overflow"))?;
        v.live_credit = Uint4::from(ValueSto::credit_u32(next_credit, "rent credit overflow")?);
        v.charge = BlockHeight::from(curhei);
        self.ctrtkvdb_set(&sk, &v);
        Ok(u64_to_i64_sat(unit).saturating_mul(period as i64))
    }

    fn srecv(
        &mut self,
        gst: &GasExtra,
        cap: &SpaceCap,
        curhei: u64,
        cadr: &Address,
        k: Value,
        p: Value,
    ) -> VmrtRes<i64> {
        let period = parse_period(p, cap.storage_recv_max_periods)?;
        let sk = Self::skey(cadr, &k, cap.value_size)?;
        let Some(mut v) = self.sfetch(curhei, gst, &sk)? else {
            return itr_err_code!(StorageKeyNotFind);
        };
        let unit = v.unit(gst)?;
        let add_credit = period_credit(unit, period, cap.storage_period)?;
        let add_blocks = period
            .checked_mul(cap.storage_period)
            .ok_or_else(|| ItrErr::new(StorageError, "recover blocks overflow"))?;
        let cur_blocks = rest_blocks(v.recover_credit.uint() as u64, unit)?;
        let next_blocks = cur_blocks
            .checked_add(add_blocks)
            .ok_or_else(|| ItrErr::new(StorageError, "recover overflow"))?;
        if next_blocks > cap.storage_recv_max_blocks() {
            return itr_err_fmt!(
                StoragePeriodErr,
                "recover block budget exceeded, max {} blocks",
                cap.storage_recv_max_blocks()
            );
        }
        let next_credit = (v.recover_credit.uint() as u64)
            .checked_add(add_credit)
            .ok_or_else(|| ItrErr::new(StorageError, "recover credit overflow"))?;
        v.recover_credit = Uint4::from(ValueSto::credit_u32(
            next_credit,
            "recover credit overflow",
        )?);
        v.charge = BlockHeight::from(curhei);
        self.ctrtkvdb_set(&sk, &v);
        Ok(u64_to_i64_sat(unit)
            .saturating_mul(period as i64)
            .saturating_div(3))
    }

    fn sdel(
        &mut self,
        gst: &GasExtra,
        cap: &SpaceCap,
        curhei: u64,
        cadr: &Address,
        k: Value,
    ) -> VmrtRes<i64> {
        let sk = Self::skey(cadr, &k, cap.value_size)?;
        let Some(mut v) = self.ctrtkvdb(&sk) else {
            return Ok(0);
        };
        v.settle(curhei, gst)?;
        if v.is_absent() {
            self.ctrtkvdb_del(&sk);
            return Ok(0);
        }
        let refund = refund_for_live_credit(v.live_credit.uint() as u64, cap.storage_period);
        self.ctrtkvdb_del(&sk);
        let refund = refund
            .checked_add(gst.storage_key_cost)
            .ok_or_else(|| ItrErr::new(StorageError, "delete refund overflow"))?;
        Ok(refund)
    }
}

#[cfg(test)]
mod storage_field_tests {
    use super::*;
    use crate::rt::ItrErrCode;
    use testkit::sim::state::FlatMemState as StateMem;

    fn test_addr() -> Address {
        Address::create_contract([1u8; 20])
    }

    fn test_gas() -> GasExtra {
        let mut gst = GasExtra::new(1);
        gst.storege_value_base_size = 0;
        gst.storage_key_cost = 11;
        gst.storage_edit_mul = 1;
        gst
    }

    fn test_cap() -> SpaceCap {
        let mut cap = SpaceCap::new(1);
        cap.value_size = 64;
        cap.kv_key_size = 128;
        cap.status_pure_size = 128;
        cap
    }

    fn overflow_credit_cap() -> SpaceCap {
        let mut cap = SpaceCap::new(1);
        cap.value_size = u16::MAX as usize - 1;
        cap.storage_period = 1;
        cap.storage_live_max_periods = Uint4::MAX as u64;
        cap.storage_recv_max_periods = Uint4::MAX as u64;
        cap
    }

    #[test]
    fn status_get_put_delete_and_isolation_work() {
        let mut state = StateMem::default();
        let mut vmsta = VMState::wrap(&mut state);
        let cap = test_cap();
        let addr = test_addr();
        let other = Address::create_contract([2u8; 20]);
        let key = Value::Bytes(b"status-key".to_vec());

        assert_eq!(vmsta.sget(&cap, &addr, &key).unwrap(), Value::Nil);
        vmsta
            .sput(&cap, &addr, key.clone(), Value::Bytes(b"abc".to_vec()))
            .unwrap();
        assert_eq!(
            vmsta.sget(&cap, &addr, &key).unwrap(),
            Value::Bytes(b"abc".to_vec())
        );
        assert_eq!(vmsta.sget(&cap, &other, &key).unwrap(), Value::Nil);
        vmsta.sput(&cap, &addr, key.clone(), Value::Nil).unwrap();
        assert_eq!(vmsta.sget(&cap, &addr, &key).unwrap(), Value::Nil);
        let caddr = ContractAddress::from_addr(addr).unwrap();
        assert!(vmsta.ctrtstatus(&caddr).is_none());
    }

    #[test]
    fn status_payload_limit_is_enforced_by_pure_key_plus_value_size() {
        let mut state = StateMem::default();
        let mut vmsta = VMState::wrap(&mut state);
        let cap = test_cap();
        let addr = test_addr();
        let key = Value::Bytes(vec![1u8; 60]);

        vmsta
            .sput(&cap, &addr, key.clone(), Value::Bytes(vec![2u8; 68]))
            .unwrap();
        let err = vmsta
            .sput(&cap, &addr, key, Value::Bytes(vec![3u8; 69]))
            .unwrap_err();
        assert_eq!(err.0, ItrErrCode::StorageValSizeErr);
    }

    #[test]
    fn status_rejects_key_longer_than_space_cap() {
        let mut state = StateMem::default();
        let mut vmsta = VMState::wrap(&mut state);
        let mut cap = test_cap();
        cap.kv_key_size = 3;
        let addr = test_addr();
        let key = Value::Bytes(b"long".to_vec());

        assert_eq!(
            vmsta.sget(&cap, &addr, &key).unwrap_err().0,
            ItrErrCode::StorageKeyInvalid
        );
        assert_eq!(
            vmsta
                .sput(&cap, &addr, key, Value::Bytes(vec![1]))
                .unwrap_err()
                .0,
            ItrErrCode::StorageKeyInvalid
        );
    }

    #[test]
    fn uint4_credit_capacity_matches_current_storage_caps() {
        let gst = GasExtra::new(1);
        let cap = SpaceCap::new(1);
        let unit = ValueSto::unit_for(&gst, &Value::Bytes(vec![0u8; cap.value_size])).unwrap();
        let live_credit =
            period_credit(unit, cap.storage_live_max_periods, cap.storage_period).unwrap();
        let recover_credit =
            period_credit(unit, cap.storage_recv_max_periods, cap.storage_period).unwrap();
        assert!(live_credit <= Uint4::MAX as u64);
        assert!(recover_credit <= Uint4::MAX as u64);
    }

    #[test]
    fn uint4_credit_overflow_is_reported_for_oversized_storage_caps() {
        let gst = test_gas();
        let cap = overflow_credit_cap();
        let addr = test_addr();
        let key = Value::Bytes(b"ovf".to_vec());
        let err = VMState::wrap(&mut StateMem::default())
            .snew(
                &gst,
                &cap,
                1,
                &addr,
                key,
                Value::Bytes(vec![0u8; cap.value_size]),
                Value::U64(cap.storage_live_max_periods),
            )
            .unwrap_err();
        assert_eq!(err.0, ItrErrCode::StorageError);
        assert!(err.1.contains("live credit overflow"));
    }

    #[test]
    fn sstat_rounds_up_live_blocks_to_match_sload() {
        let mut state = StateMem::default();
        let mut vmsta = VMState::wrap(&mut state);
        let gst = test_gas();
        let cap = test_cap();
        let addr = test_addr();
        let key = Value::Bytes(b"k1".to_vec());
        let val = Value::Bytes(vec![7, 8, 9, 10]);

        vmsta
            .snew(
                &gst,
                &cap,
                1,
                &addr,
                key.clone(),
                val.clone(),
                Value::U64(1),
            )
            .unwrap();
        let sk = VMState::skey(&addr, &key, cap.value_size).unwrap();
        let mut sto = vmsta.ctrtkvdb(&sk).unwrap();
        sto.live_credit = Uint4::from(1);
        sto.recover_credit = Uint4::from(0);
        vmsta.ctrtkvdb_set(&sk, &sto);

        let stat = vmsta.sstat(&gst, &cap, 1, &addr, &key).unwrap();
        let Value::Tuple(items) = stat else {
            panic!("expected tuple");
        };
        let vals = items.to_vec();
        assert_eq!(vals, vec![Value::U64(1), Value::U64(0)]);
        assert_eq!(vmsta.sload(&gst, &cap, 1, &addr, &key).unwrap(), val);
    }

    #[test]
    fn sstat_rounds_up_recover_blocks_to_match_not_active_state() {
        let mut state = StateMem::default();
        let mut vmsta = VMState::wrap(&mut state);
        let gst = test_gas();
        let cap = test_cap();
        let addr = test_addr();
        let key = Value::Bytes(b"k2".to_vec());
        let val = Value::Bytes(vec![1, 2, 3, 4]);

        vmsta
            .snew(&gst, &cap, 1, &addr, key.clone(), val, Value::U64(1))
            .unwrap();
        let sk = VMState::skey(&addr, &key, cap.value_size).unwrap();
        let mut sto = vmsta.ctrtkvdb(&sk).unwrap();
        sto.live_credit = Uint4::from(0);
        sto.recover_credit = Uint4::from(1);
        vmsta.ctrtkvdb_set(&sk, &sto);

        let stat = vmsta.sstat(&gst, &cap, 1, &addr, &key).unwrap();
        let Value::Tuple(items) = stat else {
            panic!("expected tuple");
        };
        let vals = items.to_vec();
        assert_eq!(vals, vec![Value::U64(0), Value::U64(1)]);
        let err = vmsta.sload(&gst, &cap, 1, &addr, &key).unwrap_err();
        assert_eq!(err.0, ItrErrCode::StorageNotActive);
    }

    #[test]
    fn sdel_refunds_key_cost_for_recoverable_but_not_expired_entries() {
        let mut state = StateMem::default();
        let mut vmsta = VMState::wrap(&mut state);
        let gst = test_gas();
        let cap = test_cap();
        let addr = test_addr();
        let key = Value::Bytes(b"k3".to_vec());
        let val = Value::Bytes(vec![1, 2, 3, 4]);

        vmsta
            .snew(&gst, &cap, 1, &addr, key.clone(), val, Value::U64(1))
            .unwrap();
        let sk = VMState::skey(&addr, &key, cap.value_size).unwrap();
        let mut sto = vmsta.ctrtkvdb(&sk).unwrap();
        sto.live_credit = Uint4::from(0);
        sto.recover_credit = Uint4::from(1);
        vmsta.ctrtkvdb_set(&sk, &sto);

        let refund = vmsta.sdel(&gst, &cap, 1, &addr, key.clone()).unwrap();
        assert_eq!(refund, gst.storage_key_cost);
        assert!(vmsta.ctrtkvdb(&sk).is_none());
    }

    #[test]
    fn sdel_returns_zero_after_entry_has_fully_expired() {
        let mut state = StateMem::default();
        let mut vmsta = VMState::wrap(&mut state);
        let gst = test_gas();
        let cap = test_cap();
        let addr = test_addr();
        let key = Value::Bytes(b"k4".to_vec());
        let val = Value::Bytes(vec![1, 2, 3, 4]);

        vmsta
            .snew(&gst, &cap, 1, &addr, key.clone(), val, Value::U64(1))
            .unwrap();
        let refund = vmsta
            .sdel(&gst, &cap, cap.storage_period * 2 + 10, &addr, key.clone())
            .unwrap();
        assert_eq!(refund, 0);
        let sk = VMState::skey(&addr, &key, cap.value_size).unwrap();
        assert!(vmsta.ctrtkvdb(&sk).is_none());
    }

    #[test]
    fn srent_and_srecv_use_rounded_up_existing_blocks_for_limits() {
        let mut state = StateMem::default();
        let mut vmsta = VMState::wrap(&mut state);
        let gst = test_gas();
        let cap = test_cap();
        let addr = test_addr();

        let key_live = Value::Bytes(b"k5".to_vec());
        vmsta
            .snew(
                &gst,
                &cap,
                1,
                &addr,
                key_live.clone(),
                Value::Bytes(vec![1, 2, 3, 4]),
                Value::U64(1),
            )
            .unwrap();
        let sk_live = VMState::skey(&addr, &key_live, cap.value_size).unwrap();
        let mut sto_live = vmsta.ctrtkvdb(&sk_live).unwrap();
        sto_live.live_credit = Uint4::from(1);
        vmsta.ctrtkvdb_set(&sk_live, &sto_live);
        let err = vmsta
            .srent(
                &gst,
                &cap,
                1,
                &addr,
                key_live,
                Value::U64(cap.storage_live_max_periods),
            )
            .unwrap_err();
        assert_eq!(err.0, ItrErrCode::StoragePeriodErr);

        let key_recv = Value::Bytes(b"k6".to_vec());
        vmsta
            .snew(
                &gst,
                &cap,
                1,
                &addr,
                key_recv.clone(),
                Value::Bytes(vec![1, 2, 3, 4]),
                Value::U64(1),
            )
            .unwrap();
        let sk_recv = VMState::skey(&addr, &key_recv, cap.value_size).unwrap();
        let mut sto_recv = vmsta.ctrtkvdb(&sk_recv).unwrap();
        sto_recv.recover_credit = Uint4::from(1);
        vmsta.ctrtkvdb_set(&sk_recv, &sto_recv);
        let err = vmsta
            .srecv(
                &gst,
                &cap,
                1,
                &addr,
                key_recv,
                Value::U64(cap.storage_recv_max_periods),
            )
            .unwrap_err();
        assert_eq!(err.0, ItrErrCode::StoragePeriodErr);
    }

    #[test]
    fn sdel_live_refund_scales_from_credit_before_key_cost() {
        let mut state = StateMem::default();
        let mut vmsta = VMState::wrap(&mut state);
        let gst = test_gas();
        let cap = test_cap();
        let addr = test_addr();
        let key = Value::Bytes(b"k7".to_vec());

        vmsta
            .snew(
                &gst,
                &cap,
                1,
                &addr,
                key.clone(),
                Value::Bytes(vec![1, 2, 3, 4]),
                Value::U64(1),
            )
            .unwrap();
        let sk = VMState::skey(&addr, &key, cap.value_size).unwrap();
        let mut sto = vmsta.ctrtkvdb(&sk).unwrap();
        sto.live_credit = Uint4::from(250);
        sto.recover_credit = Uint4::from(0);
        vmsta.ctrtkvdb_set(&sk, &sto);

        let refund = vmsta.sdel(&gst, &cap, 1, &addr, key).unwrap();
        assert_eq!(
            refund,
            refund_for_live_credit(250, cap.storage_period).saturating_add(gst.storage_key_cost)
        );
    }

    #[test]
    fn recoverable_entry_cannot_edit_until_restored_active() {
        let mut state = StateMem::default();
        let mut vmsta = VMState::wrap(&mut state);
        let gst = test_gas();
        let cap = test_cap();
        let addr = test_addr();
        let key = Value::Bytes(b"k8".to_vec());

        vmsta
            .snew(
                &gst,
                &cap,
                1,
                &addr,
                key.clone(),
                Value::Bytes(vec![1, 2, 3, 4]),
                Value::U64(1),
            )
            .unwrap();
        let sk = VMState::skey(&addr, &key, cap.value_size).unwrap();
        let mut sto = vmsta.ctrtkvdb(&sk).unwrap();
        sto.live_credit = Uint4::from(0);
        sto.recover_credit = Uint4::from(50);
        vmsta.ctrtkvdb_set(&sk, &sto);

        let err = vmsta
            .sedit(
                &gst,
                &cap,
                1,
                &addr,
                key.clone(),
                Value::Bytes(vec![9, 9, 9, 9]),
            )
            .unwrap_err();
        assert_eq!(err.0, ItrErrCode::StorageNotActive);

        vmsta
            .srent(&gst, &cap, 1, &addr, key.clone(), Value::U64(1))
            .unwrap();
        vmsta
            .sedit(
                &gst,
                &cap,
                1,
                &addr,
                key.clone(),
                Value::Bytes(vec![9, 9, 9, 9]),
            )
            .unwrap();
        assert_eq!(
            vmsta.sload(&gst, &cap, 1, &addr, &key).unwrap(),
            Value::Bytes(vec![9, 9, 9, 9])
        );
    }

    #[test]
    fn recoverable_entry_can_delete_then_recreate_same_key() {
        let mut state = StateMem::default();
        let mut vmsta = VMState::wrap(&mut state);
        let gst = test_gas();
        let cap = test_cap();
        let addr = test_addr();
        let key = Value::Bytes(b"k9".to_vec());

        vmsta
            .snew(
                &gst,
                &cap,
                1,
                &addr,
                key.clone(),
                Value::Bytes(vec![1, 2, 3, 4]),
                Value::U64(1),
            )
            .unwrap();
        let sk = VMState::skey(&addr, &key, cap.value_size).unwrap();
        let mut sto = vmsta.ctrtkvdb(&sk).unwrap();
        sto.live_credit = Uint4::from(0);
        sto.recover_credit = Uint4::from(20);
        vmsta.ctrtkvdb_set(&sk, &sto);

        let err = vmsta
            .snew(
                &gst,
                &cap,
                1,
                &addr,
                key.clone(),
                Value::Bytes(vec![7, 7, 7, 7]),
                Value::U64(1),
            )
            .unwrap_err();
        assert_eq!(err.0, ItrErrCode::StorageKeyExists);

        let refund = vmsta.sdel(&gst, &cap, 1, &addr, key.clone()).unwrap();
        assert_eq!(refund, gst.storage_key_cost);

        vmsta
            .snew(
                &gst,
                &cap,
                1,
                &addr,
                key.clone(),
                Value::Bytes(vec![7, 7, 7, 7]),
                Value::U64(1),
            )
            .unwrap();
        assert_eq!(
            vmsta.sload(&gst, &cap, 1, &addr, &key).unwrap(),
            Value::Bytes(vec![7, 7, 7, 7])
        );
    }

    #[test]
    fn sedit_shrink_clamps_live_to_cap_and_rebates_trimmed_live_only() {
        let mut state = StateMem::default();
        let mut vmsta = VMState::wrap(&mut state);
        let gst = test_gas();
        let mut cap = test_cap();
        cap.storage_period = 10;
        cap.storage_live_max_periods = 2;
        cap.storage_recv_max_periods = 2;
        let addr = test_addr();
        let key = Value::Bytes(b"k9a".to_vec());

        vmsta
            .snew(
                &gst,
                &cap,
                1,
                &addr,
                key.clone(),
                Value::Bytes(vec![1, 2, 3, 4]),
                Value::U64(1),
            )
            .unwrap();
        let sk = VMState::skey(&addr, &key, cap.value_size).unwrap();
        let mut sto = vmsta.ctrtkvdb(&sk).unwrap();
        sto.live_credit = Uint4::from(400);
        sto.recover_credit = Uint4::from(15);
        vmsta.ctrtkvdb_set(&sk, &sto);

        let (fee, rebate) = vmsta
            .sedit(&gst, &cap, 1, &addr, key.clone(), Value::Bytes(vec![9]))
            .unwrap();
        assert_eq!(fee, 1);
        assert_eq!(rebate, refund_for_live_credit(380, cap.storage_period));

        let sto = vmsta.ctrtkvdb(&sk).unwrap();
        assert_eq!(sto.live_credit.uint(), 20);
        assert_eq!(sto.recover_credit.uint(), 15);
        let stat = vmsta.sstat(&gst, &cap, 1, &addr, &key).unwrap();
        let Value::Tuple(items) = stat else {
            panic!("expected tuple")
        };
        assert_eq!(
            items.to_vec(),
            vec![Value::U64(cap.storage_live_max_blocks()), Value::U64(15)]
        );
    }

    #[test]
    fn sedit_shrink_burns_trimmed_recover_without_rebate() {
        let mut state = StateMem::default();
        let mut vmsta = VMState::wrap(&mut state);
        let gst = test_gas();
        let mut cap = test_cap();
        cap.storage_period = 10;
        cap.storage_live_max_periods = 3;
        cap.storage_recv_max_periods = 2;
        let addr = test_addr();
        let key = Value::Bytes(b"k9b".to_vec());

        vmsta
            .snew(
                &gst,
                &cap,
                1,
                &addr,
                key.clone(),
                Value::Bytes(vec![1, 2, 3, 4]),
                Value::U64(1),
            )
            .unwrap();
        let sk = VMState::skey(&addr, &key, cap.value_size).unwrap();
        let mut sto = vmsta.ctrtkvdb(&sk).unwrap();
        sto.live_credit = Uint4::from(25);
        sto.recover_credit = Uint4::from(200);
        vmsta.ctrtkvdb_set(&sk, &sto);

        let (_, rebate) = vmsta
            .sedit(&gst, &cap, 1, &addr, key.clone(), Value::Bytes(vec![7]))
            .unwrap();
        assert_eq!(rebate, 0);

        let sto = vmsta.ctrtkvdb(&sk).unwrap();
        assert_eq!(sto.live_credit.uint(), 25);
        assert_eq!(sto.recover_credit.uint(), 20);
        let stat = vmsta.sstat(&gst, &cap, 1, &addr, &key).unwrap();
        let Value::Tuple(items) = stat else {
            panic!("expected tuple")
        };
        assert_eq!(
            items.to_vec(),
            vec![Value::U64(25), Value::U64(cap.storage_recv_max_blocks())]
        );
    }

    #[test]
    fn sedit_grow_shortens_life_without_forcing_top_up() {
        let mut state = StateMem::default();
        let mut vmsta = VMState::wrap(&mut state);
        let gst = test_gas();
        let mut cap = test_cap();
        cap.storage_period = 10;
        let addr = test_addr();
        let key = Value::Bytes(b"k9c".to_vec());

        vmsta
            .snew(
                &gst,
                &cap,
                1,
                &addr,
                key.clone(),
                Value::Bytes(vec![1]),
                Value::U64(1),
            )
            .unwrap();
        let sk = VMState::skey(&addr, &key, cap.value_size).unwrap();
        let mut sto = vmsta.ctrtkvdb(&sk).unwrap();
        sto.live_credit = Uint4::from(20);
        sto.recover_credit = Uint4::from(20);
        vmsta.ctrtkvdb_set(&sk, &sto);

        let (_, rebate) = vmsta
            .sedit(
                &gst,
                &cap,
                1,
                &addr,
                key.clone(),
                Value::Bytes(vec![1, 2, 3, 4, 5]),
            )
            .unwrap();
        assert_eq!(rebate, 0);

        let stat = vmsta.sstat(&gst, &cap, 1, &addr, &key).unwrap();
        let Value::Tuple(items) = stat else {
            panic!("expected tuple")
        };
        assert_eq!(items.to_vec(), vec![Value::U64(4), Value::U64(4)]);
    }

    #[test]
    fn sedit_shrink_at_cap_has_no_rebate() {
        let mut state = StateMem::default();
        let mut vmsta = VMState::wrap(&mut state);
        let gst = test_gas();
        let mut cap = test_cap();
        cap.storage_period = 10;
        cap.storage_live_max_periods = 2;
        cap.storage_recv_max_periods = 2;
        let addr = test_addr();
        let key = Value::Bytes(b"k9d".to_vec());

        vmsta
            .snew(
                &gst,
                &cap,
                1,
                &addr,
                key.clone(),
                Value::Bytes(vec![1, 2, 3, 4]),
                Value::U64(1),
            )
            .unwrap();
        let sk = VMState::skey(&addr, &key, cap.value_size).unwrap();
        let mut sto = vmsta.ctrtkvdb(&sk).unwrap();
        sto.live_credit = Uint4::from(20);
        sto.recover_credit = Uint4::from(20);
        vmsta.ctrtkvdb_set(&sk, &sto);

        let (_, rebate) = vmsta
            .sedit(&gst, &cap, 1, &addr, key.clone(), Value::Bytes(vec![1]))
            .unwrap();
        assert_eq!(rebate, 0);

        let sto = vmsta.ctrtkvdb(&sk).unwrap();
        assert_eq!(sto.live_credit.uint(), 20);
        assert_eq!(sto.recover_credit.uint(), 20);
    }

    #[test]
    fn recoverable_entry_can_extend_recover_and_restore_later() {
        let mut state = StateMem::default();
        let mut vmsta = VMState::wrap(&mut state);
        let gst = test_gas();
        let cap = test_cap();
        let addr = test_addr();
        let key = Value::Bytes(b"k10".to_vec());

        vmsta
            .snew(
                &gst,
                &cap,
                1,
                &addr,
                key.clone(),
                Value::Bytes(vec![3, 3, 3, 3]),
                Value::U64(1),
            )
            .unwrap();
        let sk = VMState::skey(&addr, &key, cap.value_size).unwrap();
        let mut sto = vmsta.ctrtkvdb(&sk).unwrap();
        sto.live_credit = Uint4::from(0);
        sto.recover_credit = Uint4::from(1);
        vmsta.ctrtkvdb_set(&sk, &sto);

        let before = vmsta.sstat(&gst, &cap, 1, &addr, &key).unwrap();
        vmsta
            .srecv(&gst, &cap, 1, &addr, key.clone(), Value::U64(1))
            .unwrap();
        let after = vmsta.sstat(&gst, &cap, 1, &addr, &key).unwrap();
        assert_ne!(before, after);

        let err = vmsta.sload(&gst, &cap, 1, &addr, &key).unwrap_err();
        assert_eq!(err.0, ItrErrCode::StorageNotActive);

        vmsta
            .srent(&gst, &cap, 1, &addr, key.clone(), Value::U64(1))
            .unwrap();
        assert_eq!(
            vmsta.sload(&gst, &cap, 1, &addr, &key).unwrap(),
            Value::Bytes(vec![3, 3, 3, 3])
        );
    }

    #[test]
    fn storage_state_matrix_matches_business_rules() {
        let gst = test_gas();
        let cap = test_cap();
        let addr = test_addr();

        // active: readable, editable, not recreatable, rentable, recover-rentable, deletable
        {
            let mut state = StateMem::default();
            let mut vmsta = VMState::wrap(&mut state);
            let key = Value::Bytes(b"m1".to_vec());
            let val = Value::Bytes(vec![1, 1, 1, 1]);
            vmsta
                .snew(
                    &gst,
                    &cap,
                    1,
                    &addr,
                    key.clone(),
                    val.clone(),
                    Value::U64(1),
                )
                .unwrap();

            assert_eq!(vmsta.sload(&gst, &cap, 1, &addr, &key).unwrap(), val);
            vmsta
                .sedit(
                    &gst,
                    &cap,
                    1,
                    &addr,
                    key.clone(),
                    Value::Bytes(vec![2, 2, 2, 2]),
                )
                .unwrap();
            assert_eq!(
                vmsta
                    .snew(
                        &gst,
                        &cap,
                        1,
                        &addr,
                        key.clone(),
                        Value::Bytes(vec![9, 9, 9, 9]),
                        Value::U64(1)
                    )
                    .unwrap_err()
                    .0,
                ItrErrCode::StorageKeyExists
            );
            vmsta
                .srent(&gst, &cap, 1, &addr, key.clone(), Value::U64(1))
                .unwrap();
            vmsta
                .srecv(&gst, &cap, 1, &addr, key.clone(), Value::U64(1))
                .unwrap();
            assert!(vmsta.sdel(&gst, &cap, 1, &addr, key).unwrap() >= gst.storage_key_cost);
        }

        // recoverable: not readable, not editable, not recreatable, rentable, recover-rentable, deletable
        {
            let mut state = StateMem::default();
            let mut vmsta = VMState::wrap(&mut state);
            let key = Value::Bytes(b"m2".to_vec());
            vmsta
                .snew(
                    &gst,
                    &cap,
                    1,
                    &addr,
                    key.clone(),
                    Value::Bytes(vec![3, 3, 3, 3]),
                    Value::U64(1),
                )
                .unwrap();
            let sk = VMState::skey(&addr, &key, cap.value_size).unwrap();
            let mut sto = vmsta.ctrtkvdb(&sk).unwrap();
            sto.live_credit = Uint4::from(0);
            sto.recover_credit = Uint4::from(10);
            vmsta.ctrtkvdb_set(&sk, &sto);

            assert_eq!(
                vmsta.sload(&gst, &cap, 1, &addr, &key).unwrap_err().0,
                ItrErrCode::StorageNotActive
            );
            assert_eq!(
                vmsta
                    .sedit(
                        &gst,
                        &cap,
                        1,
                        &addr,
                        key.clone(),
                        Value::Bytes(vec![4, 4, 4, 4])
                    )
                    .unwrap_err()
                    .0,
                ItrErrCode::StorageNotActive
            );
            assert_eq!(
                vmsta
                    .snew(
                        &gst,
                        &cap,
                        1,
                        &addr,
                        key.clone(),
                        Value::Bytes(vec![9, 9, 9, 9]),
                        Value::U64(1)
                    )
                    .unwrap_err()
                    .0,
                ItrErrCode::StorageKeyExists
            );
            vmsta
                .srecv(&gst, &cap, 1, &addr, key.clone(), Value::U64(1))
                .unwrap();
            assert_eq!(
                vmsta.sdel(&gst, &cap, 1, &addr, key.clone()).unwrap(),
                gst.storage_key_cost
            );

            vmsta
                .snew(
                    &gst,
                    &cap,
                    1,
                    &addr,
                    key.clone(),
                    Value::Bytes(vec![3, 3, 3, 3]),
                    Value::U64(1),
                )
                .unwrap();
            let sk = VMState::skey(&addr, &key, cap.value_size).unwrap();
            let mut sto = vmsta.ctrtkvdb(&sk).unwrap();
            sto.live_credit = Uint4::from(0);
            sto.recover_credit = Uint4::from(10);
            vmsta.ctrtkvdb_set(&sk, &sto);
            vmsta
                .srent(&gst, &cap, 1, &addr, key.clone(), Value::U64(1))
                .unwrap();
            assert!(vmsta.sdel(&gst, &cap, 1, &addr, key).unwrap() > gst.storage_key_cost);
        }

        // absent: nil on read/stat, no edit/rent/recover, recreatable, delete returns 0
        {
            let mut state = StateMem::default();
            let mut vmsta = VMState::wrap(&mut state);
            let key = Value::Bytes(b"m3".to_vec());
            let val = Value::Bytes(vec![5, 5, 5, 5]);
            vmsta
                .snew(
                    &gst,
                    &cap,
                    1,
                    &addr,
                    key.clone(),
                    val.clone(),
                    Value::U64(1),
                )
                .unwrap();

            assert_eq!(
                vmsta
                    .sload(&gst, &cap, cap.storage_period * 2 + 10, &addr, &key)
                    .unwrap(),
                Value::Nil
            );
            assert_eq!(
                vmsta
                    .sstat(&gst, &cap, cap.storage_period * 2 + 10, &addr, &key)
                    .unwrap(),
                Value::Nil
            );
            assert_eq!(
                vmsta
                    .sedit(
                        &gst,
                        &cap,
                        cap.storage_period * 2 + 10,
                        &addr,
                        key.clone(),
                        Value::Bytes(vec![6, 6, 6, 6])
                    )
                    .unwrap_err()
                    .0,
                ItrErrCode::StorageKeyNotFind
            );
            assert_eq!(
                vmsta
                    .srent(
                        &gst,
                        &cap,
                        cap.storage_period * 2 + 10,
                        &addr,
                        key.clone(),
                        Value::U64(1)
                    )
                    .unwrap_err()
                    .0,
                ItrErrCode::StorageKeyNotFind
            );
            assert_eq!(
                vmsta
                    .srecv(
                        &gst,
                        &cap,
                        cap.storage_period * 2 + 10,
                        &addr,
                        key.clone(),
                        Value::U64(1)
                    )
                    .unwrap_err()
                    .0,
                ItrErrCode::StorageKeyNotFind
            );
            assert_eq!(
                vmsta
                    .sdel(&gst, &cap, cap.storage_period * 2 + 10, &addr, key.clone())
                    .unwrap(),
                0
            );
            vmsta
                .snew(
                    &gst,
                    &cap,
                    cap.storage_period * 2 + 10,
                    &addr,
                    key.clone(),
                    val.clone(),
                    Value::U64(1),
                )
                .unwrap();
            assert_eq!(
                vmsta
                    .sload(&gst, &cap, cap.storage_period * 2 + 10, &addr, &key)
                    .unwrap(),
                val
            );
        }
    }

    #[test]
    fn lazy_settlement_transitions_active_to_recoverable_to_absent() {
        let mut state = StateMem::default();
        let mut vmsta = VMState::wrap(&mut state);
        let gst = test_gas();
        let cap = test_cap();
        let addr = test_addr();
        let key = Value::Bytes(b"lz1".to_vec());
        let val = Value::Bytes(vec![8, 8, 8, 8]);

        vmsta
            .snew(
                &gst,
                &cap,
                1,
                &addr,
                key.clone(),
                val.clone(),
                Value::U64(1),
            )
            .unwrap();
        let sk = VMState::skey(&addr, &key, cap.value_size).unwrap();
        let mut sto = vmsta.ctrtkvdb(&sk).unwrap();
        sto.live_credit = Uint4::from(5);
        sto.recover_credit = Uint4::from(5);
        vmsta.ctrtkvdb_set(&sk, &sto);

        assert_eq!(
            vmsta.sload(&gst, &cap, 1, &addr, &key).unwrap(),
            val,
            "initially active"
        );

        let err = vmsta.sload(&gst, &cap, 3, &addr, &key).unwrap_err();
        assert_eq!(
            err.0,
            ItrErrCode::StorageNotActive,
            "live credit should burn first into recoverable"
        );
        let stat = vmsta.sstat(&gst, &cap, 3, &addr, &key).unwrap();
        let Value::Tuple(items) = stat else {
            panic!("expected tuple")
        };
        assert_eq!(items.to_vec(), vec![Value::U64(0), Value::U64(1)]);

        assert_eq!(
            vmsta.sload(&gst, &cap, 4, &addr, &key).unwrap(),
            Value::Nil,
            "after recover burns out the entry becomes absent"
        );
        assert!(
            vmsta.ctrtkvdb(&sk).is_none(),
            "lazy settle should physically delete absent entry on access"
        );
    }

    #[test]
    fn lazy_settlement_only_applies_when_entry_is_accessed() {
        let mut state = StateMem::default();
        let mut vmsta = VMState::wrap(&mut state);
        let gst = test_gas();
        let cap = test_cap();
        let addr = test_addr();
        let key = Value::Bytes(b"lz2".to_vec());
        let val = Value::Bytes(vec![4, 4, 4, 4]);

        vmsta
            .snew(
                &gst,
                &cap,
                1,
                &addr,
                key.clone(),
                val.clone(),
                Value::U64(1),
            )
            .unwrap();
        let sk = VMState::skey(&addr, &key, cap.value_size).unwrap();
        let mut sto = vmsta.ctrtkvdb(&sk).unwrap();
        sto.live_credit = Uint4::from(5);
        sto.recover_credit = Uint4::from(5);
        vmsta.ctrtkvdb_set(&sk, &sto);

        let stored = vmsta.ctrtkvdb(&sk).unwrap();
        assert_eq!(
            stored.charge.uint(),
            1,
            "without access, lazy settlement should not advance charge height"
        );
        assert_eq!(stored.live_credit.uint(), 5);
        assert_eq!(stored.recover_credit.uint(), 5);

        let _ = vmsta.sstat(&gst, &cap, 3, &addr, &key).unwrap();
        let settled = vmsta.ctrtkvdb(&sk).unwrap();
        assert_eq!(
            settled.charge.uint(),
            3,
            "access should trigger settlement and persist new charge height"
        );
        assert_eq!(settled.live_credit.uint(), 0);
        assert_eq!(settled.recover_credit.uint(), 2);
    }

    #[test]
    fn storage_rejects_nil_and_empty_keys_across_entry_points() {
        let gst = test_gas();
        let cap = test_cap();
        let addr = test_addr();

        for key in [Value::Nil, Value::Bytes(vec![])] {
            let mut state = StateMem::default();
            let mut vmsta = VMState::wrap(&mut state);

            assert_eq!(
                vmsta.sstat(&gst, &cap, 1, &addr, &key).unwrap_err().0,
                ItrErrCode::StorageKeyInvalid
            );
            assert_eq!(
                vmsta.sload(&gst, &cap, 1, &addr, &key).unwrap_err().0,
                ItrErrCode::StorageKeyInvalid
            );
            assert_eq!(
                vmsta
                    .snew(
                        &gst,
                        &cap,
                        1,
                        &addr,
                        key.clone(),
                        Value::Bytes(vec![1]),
                        Value::U64(1)
                    )
                    .unwrap_err()
                    .0,
                ItrErrCode::StorageKeyInvalid
            );
            assert_eq!(
                vmsta
                    .sedit(&gst, &cap, 1, &addr, key.clone(), Value::Bytes(vec![1]))
                    .unwrap_err()
                    .0,
                ItrErrCode::StorageKeyInvalid
            );
            assert_eq!(
                vmsta
                    .srent(&gst, &cap, 1, &addr, key.clone(), Value::U64(1))
                    .unwrap_err()
                    .0,
                ItrErrCode::StorageKeyInvalid
            );
            assert_eq!(
                vmsta
                    .srecv(&gst, &cap, 1, &addr, key.clone(), Value::U64(1))
                    .unwrap_err()
                    .0,
                ItrErrCode::StorageKeyInvalid
            );
            assert_eq!(
                vmsta.sdel(&gst, &cap, 1, &addr, key.clone()).unwrap_err().0,
                ItrErrCode::StorageKeyInvalid
            );
        }
    }
}
