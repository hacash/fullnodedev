/* contract loader */

#[derive(Debug, Clone)]
pub struct ResolvedFn {
    pub owner: ContractAddress,
    pub fnobj: Arc<FnObj>,
    pub lib_table: Arc<[Address]>,
}

#[derive(Debug, Clone)]
pub struct ResolvedCallPlan {
    pub next_bindings: FrameBindings,
    pub fnobj: Arc<FnObj>,
}

impl Resoure {
    #[inline(always)]
    fn require_resolved(found: Option<ResolvedFn>) -> VmrtRes<ResolvedFn> {
        use ItrErrCode::*;
        let Some(got) = found else {
            return itr_err_code!(CallNotExist);
        };
        Ok(got)
    }

    fn build_resolved(
        owner: &ContractAddress,
        fnobj: Arc<FnObj>,
        csto: &ContractObj,
    ) -> ResolvedFn {
        ResolvedFn {
            owner: owner.clone(),
            fnobj,
            lib_table: csto
                .sto
                .library
                .as_list()
                .iter()
                .map(|addr| addr.to_addr())
                .collect::<Vec<_>>()
                .into(),
        }
    }

    fn load_contract_from_state(
        &mut self,
        addr: &ContractAddress,
        state_ed: &ContractEdition,
        csto: ContractSto,
    ) -> VmrtRes<Arc<ContractObj>> {
        use ItrErrCode::*;
        let cobj = Arc::new(csto.into_obj()?);
        if cobj.edition != *state_ed {
            return itr_err_fmt!(
                ContractError,
                "contract edition mismatch {}",
                addr.to_readable()
            );
        }
        Ok(cobj)
    }

    fn resolve_contract<H: VmHost + ?Sized>(
        &mut self,
        host: &mut H,
        addr: &ContractAddress,
    ) -> VmrtRes<Arc<ContractObj>> {
        use ItrErrCode::*;
        let Some(state_ed) = host.contract_edition(addr) else {
            return itr_err_fmt!(
                NotFindContract,
                "cannot find contract edition {}",
                addr.to_readable()
            );
        };
        if let Some(c) = self.warm.contracts.get(addr) {
            if c.edition == state_ed {
                return Ok(c.clone());
            }
            self.warm.contracts.remove(addr);
        }
        if self.warm.contracts.len() >= self.warm.space_cap.loaded_contract {
            return itr_err_code!(OutOfLoadContract);
        }
        let cbytes = state_ed.raw_size.uint() as usize;
        if let Some(obj) = global_machine_manager()
            .contract_cache()
            .get(addr, &state_ed)
        {
            // OutOfGas here is terminal for the VM call; warmup is only recorded after gas settlement succeeds.
            self.settle_new_contract_load_gas(host, cbytes)?;
            self.warm.contracts.insert(addr.clone(), obj.clone());
            return Ok(obj);
        }
        let Some(csto) = host.contract(addr) else {
            return itr_err_fmt!(
                NotFindContract,
                "cannot find contract {}",
                addr.to_readable()
            );
        };
        let cobj = self.load_contract_from_state(addr, &state_ed, csto)?;
        // Keep this order explicit: even on miss, warmup/cache write is gated by successful gas settlement.
        self.settle_new_contract_load_gas(host, cbytes)?;
        self.warm.contracts.insert(addr.clone(), cobj.clone());
        global_machine_manager()
            .contract_cache()
            .insert(addr, cobj.clone());
        Ok(cobj)
    }

    pub fn load_contract<H: VmHost + ?Sized>(
        &mut self,
        host: &mut H,
        addr: &ContractAddress,
    ) -> VmrtRes<Arc<ContractObj>> {
        self.resolve_contract(host, addr)
    }

    fn resolve_user_on_owner<H: VmHost + ?Sized>(
        &mut self,
        host: &mut H,
        owner: &ContractAddress,
        selector: FnSign,
    ) -> VmrtRes<Option<ResolvedFn>> {
        let csto = self.resolve_contract(host, owner)?;
        let Some(fnobj) = csto.userfns.get(&selector).cloned() else {
            return Ok(None);
        };
        Ok(Some(Self::build_resolved(owner, fnobj, csto.as_ref())))
    }

    fn resolve_abst_on_owner<H: VmHost + ?Sized>(
        &mut self,
        host: &mut H,
        owner: &ContractAddress,
        selector: AbstCall,
    ) -> VmrtRes<Option<ResolvedFn>> {
        let csto = self.resolve_contract(host, owner)?;
        let Some(fnobj) = csto.abstfns.get(&selector).cloned() else {
            return Ok(None);
        };
        Ok(Some(Self::build_resolved(owner, fnobj, csto.as_ref())))
    }

    pub fn resolve_abstfn<H: VmHost + ?Sized>(
        &mut self,
        host: &mut H,
        addr: &ContractAddress,
        scty: AbstCall,
    ) -> VmrtRes<Option<ResolvedFn>> {
        if let Some(found) = self.resolve_abst_on_owner(host, addr, scty)? {
            return Ok(Some(found));
        }
        let csto = self.resolve_contract(host, addr)?;
        for parent in csto.sto.inherit.as_list() {
            if let Some(found) = self.resolve_abst_on_owner(host, parent, scty)? {
                return Ok(Some(found));
            }
        }
        Ok(None)
    }

    fn resolve_lookup_candidates<H: VmHost + ?Sized>(
        &mut self,
        host: &mut H,
        anchor: &ContractAddress,
        call: &CallSpec,
    ) -> VmrtRes<Vec<ContractAddress>> {
        let parents = if call.needs_inherit_chain() {
            self.resolve_contract(host, anchor)?
                .sto
                .inherit
                .as_list()
                .to_vec()
        } else {
            vec![]
        };
        Ok(call.resolve_candidates(anchor, &parents))
    }

    fn resolve_user_call<H: VmHost + ?Sized>(
        &mut self,
        host: &mut H,
        call: &CallSpec,
        bindings: &FrameBindings,
    ) -> VmrtRes<(ContractAddress, ResolvedFn)> {
        let anchor = call.resolve_anchor(bindings)?;
        let entries = self.resolve_lookup_candidates(host, &anchor, call)?;
        let mut found = None;
        for owner in entries {
            if let Some(hit) = self.resolve_user_on_owner(host, &owner, call.selector())? {
                found = Some(hit);
                break;
            }
        }
        Ok((anchor, Self::require_resolved(found)?))
    }

    #[cfg(feature = "calcfunc")]
    pub fn resolve_local_calcfn<H: VmHost + ?Sized>(
        &mut self,
        host: &mut H,
        owner: &ContractAddress,
        selector: FnSign,
    ) -> VmrtRes<Arc<CalcFnObj>> {
        let csto = self.resolve_contract(host, owner)?;
        csto.calcfns
            .get(&selector)
            .cloned()
            .ok_or_else(|| ItrErr::code(ItrErrCode::CallNotExist))
    }

    pub fn plan_user_call<H: VmHost + ?Sized>(
        &mut self,
        host: &mut H,
        call: &CallSpec,
        bindings: &FrameBindings,
    ) -> VmrtRes<ResolvedCallPlan> {
        let (anchor, hit) = self.resolve_user_call(host, call, bindings)?;
        if call.requires_external_visibility() && !hit.fnobj.check_conf(FnConf::External) {
            let vis = &anchor;
            let owner = &hit.owner;
            let impl_in = maybe!(
                vis == owner,
                s!(""),
                format!(" (impl in {})", owner.to_readable())
            );
            return itr_err_fmt!(
                CallNotExternal,
                "contract {}{} func sign {}",
                vis.to_readable(),
                impl_in,
                hex::encode(call.selector())
            );
        }
        let next_bindings =
            bindings.next_after_call(call.switches_context(), anchor, hit.owner, hit.lib_table);
        Ok(ResolvedCallPlan {
            next_bindings,
            fnobj: hit.fnobj,
        })
    }
}

#[cfg(test)]
mod loader_tests {
    use super::*;
    use field::{Address, Uint4};
    use sys::XRet;
    use testkit::sim::state::FlatMemState as StateMem;

    fn test_contract(base: &Address, nonce: u32) -> ContractAddress {
        ContractAddress::calculate(base, &Uint4::from(nonce))
    }

    struct StateHost {
        state: StateMem,
        gas_remaining: i64,
    }

    impl VmHost for StateHost {
        fn height(&self) -> u64 {
            1
        }

        fn main_entry_bindings(&self) -> FrameBindings {
            FrameBindings::root(Address::default(), Vec::<Address>::new().into())
        }

        fn gas_remaining(&self) -> i64 {
            self.gas_remaining
        }

        fn gas_charge(&mut self, gas: i64) -> VmrtErr {
            if gas < 0 {
                return itr_err_fmt!(GasError, "gas cost invalid: {}", gas);
            }
            self.gas_remaining -= gas;
            if self.gas_remaining < 0 {
                return itr_err_code!(OutOfGas);
            }
            Ok(())
        }

        fn gas_rebate(&mut self, gas: i64) -> VmrtErr {
            let _ = gas;
            Ok(())
        }

        fn contract_edition(&mut self, addr: &ContractAddress) -> Option<ContractEdition> {
            VMState::wrap(&mut self.state).contract_edition(addr)
        }

        fn contract(&mut self, addr: &ContractAddress) -> Option<ContractSto> {
            VMState::wrap(&mut self.state).contract(addr)
        }

        fn action_call(&mut self, _: u16, _: Vec<u8>) -> XRet<(u32, Vec<u8>)> {
            unreachable!()
        }

        fn log_push(&mut self, _: &Address, _: Vec<Value>) -> VmrtErr {
            unreachable!()
        }

        fn sstat(&mut self, _: &GasExtra, _: &SpaceCap, _: &Address, _: &Value) -> VmrtRes<Value> {
            unreachable!()
        }

        fn sload(&mut self, _: &GasExtra, _: &SpaceCap, _: &Address, _: &Value) -> VmrtRes<Value> {
            unreachable!()
        }

        fn sdel(&mut self, _: &GasExtra, _: &SpaceCap, _: &Address, _: Value) -> VmrtRes<i64> {
            unreachable!()
        }

        fn snew(
            &mut self,
            _: &GasExtra,
            _: &SpaceCap,
            _: &Address,
            _: Value,
            _: Value,
            _: Value,
        ) -> VmrtRes<i64> {
            unreachable!()
        }

        fn sedit(&mut self, _: &GasExtra, _: &SpaceCap, _: &Address, _: Value, _: Value) -> VmrtRes<i64> {
            unreachable!()
        }

        fn srent(&mut self, _: &GasExtra, _: &SpaceCap, _: &Address, _: Value, _: Value) -> VmrtRes<i64> {
            unreachable!()
        }

        fn srecv(&mut self, _: &GasExtra, _: &SpaceCap, _: &Address, _: Value, _: Value) -> VmrtRes<i64> {
            unreachable!()
        }
    }

    #[test]
    fn out_of_gas_on_cold_load_does_not_warm_tx_cache() {
        let base = Address::from_readable("1MzNY1oA3kfgYi75zquj3SRUPYztzXHzK9").unwrap();
        let caddr = test_contract(&base, 700_001);
        let csto = ContractSto::new();

        let mut state = StateMem::default();
        VMState::wrap(&mut state).contract_set_sync_edition(&caddr, &csto);
        let mut host = StateHost {
            state,
            gas_remaining: 0,
        };
        let mut res = Resoure::create(1);
        let err = match res.load_contract(&mut host, &caddr) {
            Ok(_) => panic!("expected OutOfGas"),
            Err(e) => e,
        };
        assert_eq!(err.0, ItrErrCode::OutOfGas);
        assert!(!res.warm.contracts.contains_key(&caddr));
    }

    #[test]
    fn cold_load_charges_before_warming_and_hit_is_free() {
        let base = Address::from_readable("1MzNY1oA3kfgYi75zquj3SRUPYztzXHzK9").unwrap();
        let caddr = test_contract(&base, 700_002);
        let csto = ContractSto::new();
        let cbytes = csto.calc_edition().raw_size.uint() as usize;

        let mut state = StateMem::default();
        VMState::wrap(&mut state).contract_set_sync_edition(&caddr, &csto);
        let mut host = StateHost {
            state,
            gas_remaining: 10_000,
        };
        let mut res = Resoure::create(1);
        let one_cold_fee = res.warm.gas_extra.new_contract_load
            + res.warm.gas_extra.contract_bytes(cbytes);
        {
            let _ = res.load_contract(&mut host, &caddr).unwrap();
        }
        assert!(res.warm.contracts.contains_key(&caddr));
        assert_eq!(host.gas_remaining, 10_000 - one_cold_fee);

        let gas_after_first = host.gas_remaining;
        {
            let _ = res.load_contract(&mut host, &caddr).unwrap();
        }
        assert_eq!(host.gas_remaining, gas_after_first);
    }
}
