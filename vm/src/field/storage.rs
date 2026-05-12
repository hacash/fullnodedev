combi_struct! { ValueSto,
    charge: BlockHeight
    live_credit: Uint4
    recover_credit: Uint4
    data: Value
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
    if storage_period == 0 {
        return itr_err_code!(StoragePeriodErr);
    }
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
    u64_to_i64_sat(credit.checked_div(storage_period).unwrap_or(0))
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

    /// `status_pure_size`-aggregate tests need per-value headroom beyond [`test_cap`]’s small `value_size`.
    fn test_cap_status_aggregate() -> SpaceCap {
        let mut cap = SpaceCap::new(1);
        cap.value_size = 1280;
        cap.kv_key_size = 128;
        cap.status_pure_size = 128;
        cap
    }

    #[test]
    fn status_put_prepare_rejects_key_over_cap_before_value_checks() {
        let cap = SpaceCap::new(1);
        let key = Value::Bytes(vec![0xAB; cap.kv_key_size + 1]);
        let v = Value::U8(1);
        assert_eq!(
            VMState::status_put_prepare(&cap, &key, &v)
                .unwrap_err()
                .0,
            ItrErrCode::StorageKeyInvalid
        );
    }

    #[test]
    fn status_put_prepare_rejects_non_scalar_value() {
        let cap = SpaceCap::new(1);
        let key = Value::Bytes(b"k".to_vec());
        let v = Value::Tuple(
            crate::value::TupleItem::new(vec![Value::U8(1)]).unwrap(),
        );
        assert_eq!(
            VMState::status_put_prepare(&cap, &key, &v)
                .unwrap_err()
                .0,
            ItrErrCode::CastBeValueFail
        );
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
        let cap = test_cap_status_aggregate();
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
    fn status_rejects_single_value_over_cap_value_size() {
        let mut state = StateMem::default();
        let mut vmsta = VMState::wrap(&mut state);
        let cap = test_cap();
        let addr = test_addr();
        let key = Value::Bytes(b"k".to_vec());
        let val = Value::Bytes(vec![7u8; 65]);
        assert_eq!(
            vmsta
                .sput(&cap, &addr, key, val)
                .unwrap_err()
                .0,
            ItrErrCode::StorageValSizeErr
        );
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
    fn persistent_storage_rejects_key_between_kv_key_size_and_value_size() {
        let gst = test_gas();
        let cap = SpaceCap::new(1);
        assert_eq!(cap.kv_key_size, 128);
        assert!(cap.value_size > cap.kv_key_size);
        let addr = test_addr();
        let key = Value::Bytes(vec![0xAB; 129]);
        let val = Value::Bytes(vec![1u8; 8]);
        let err = VMState::wrap(&mut StateMem::default())
            .snew(&gst, &cap, 1, &addr, key.clone(), val.clone(), Value::U64(1))
            .unwrap_err();
        assert_eq!(err.0, ItrErrCode::StorageKeyInvalid);
        let err = VMState::wrap(&mut StateMem::default())
            .sload(&gst, &cap, 1, &addr, &key)
            .unwrap_err();
        assert_eq!(err.0, ItrErrCode::StorageKeyInvalid);
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
        let sk = VMState::skey(&addr, &key, cap.kv_key_size).unwrap();
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
        let sk = VMState::skey(&addr, &key, cap.kv_key_size).unwrap();
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
        let sk = VMState::skey(&addr, &key, cap.kv_key_size).unwrap();
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
        let sk = VMState::skey(&addr, &key, cap.kv_key_size).unwrap();
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
        let sk_live = VMState::skey(&addr, &key_live, cap.kv_key_size).unwrap();
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
        let sk_recv = VMState::skey(&addr, &key_recv, cap.kv_key_size).unwrap();
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
        let sk = VMState::skey(&addr, &key, cap.kv_key_size).unwrap();
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
        let sk = VMState::skey(&addr, &key, cap.kv_key_size).unwrap();
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
        let sk = VMState::skey(&addr, &key, cap.kv_key_size).unwrap();
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
        let sk = VMState::skey(&addr, &key, cap.kv_key_size).unwrap();
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
        let sk = VMState::skey(&addr, &key, cap.kv_key_size).unwrap();
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
        let sk = VMState::skey(&addr, &key, cap.kv_key_size).unwrap();
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
        let sk = VMState::skey(&addr, &key, cap.kv_key_size).unwrap();
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
        let sk = VMState::skey(&addr, &key, cap.kv_key_size).unwrap();
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
            let sk = VMState::skey(&addr, &key, cap.kv_key_size).unwrap();
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
            let sk = VMState::skey(&addr, &key, cap.kv_key_size).unwrap();
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
        let sk = VMState::skey(&addr, &key, cap.kv_key_size).unwrap();
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
        let sk = VMState::skey(&addr, &key, cap.kv_key_size).unwrap();
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
