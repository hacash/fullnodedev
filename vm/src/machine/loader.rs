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
        vmsta: &mut VMState,
        addr: &ContractAddress,
        state_ed: &ContractEdition,
    ) -> VmrtRes<Arc<ContractObj>> {
        use ItrErrCode::*;
        let Some(c) = vmsta.contract(addr) else {
            return itr_err_fmt!(
                NotFindContract,
                "cannot find contract {}",
                addr.to_readable()
            );
        };
        let cobj = Arc::new(c.into_obj()?);
        if cobj.edition != *state_ed {
            return itr_err_fmt!(
                ContractError,
                "contract edition mismatch {}",
                addr.to_readable()
            );
        }
        Ok(cobj)
    }

    fn resolve_contract(
        &mut self,
        vmsta: &mut VMState,
        gas: &mut i64,
        addr: &ContractAddress,
    ) -> VmrtRes<Arc<ContractObj>> {
        use ItrErrCode::*;
        let Some(state_ed) = vmsta.contract_edition(addr) else {
            return itr_err_fmt!(
                NotFindContract,
                "cannot find contract edition {}",
                addr.to_readable()
            );
        };
        if let Some(c) = self.contracts.get(addr) {
            if c.edition == state_ed {
                return Ok(c.clone());
            }
            self.contracts.remove(addr);
        }
        if self.contracts.len() >= self.space_cap.loaded_contract {
            return itr_err_code!(OutOfLoadContract);
        }
        let cbytes = state_ed.raw_size.uint() as usize;
        if let Some(obj) = global_machine_manager()
            .contract_cache()
            .get(addr, &state_ed)
        {
            // OutOfGas here is terminal for the VM call; warmup is only recorded after gas settlement succeeds.
            self.settle_new_contract_load_gas(gas, cbytes)?;
            self.contracts.insert(addr.clone(), obj.clone());
            return Ok(obj);
        }
        let cobj = self.load_contract_from_state(vmsta, addr, &state_ed)?;
        // Keep this order explicit: even on miss, warmup/cache write is gated by successful gas settlement.
        self.settle_new_contract_load_gas(gas, cbytes)?;
        self.contracts.insert(addr.clone(), cobj.clone());
        global_machine_manager()
            .contract_cache()
            .insert(addr, cobj.clone());
        Ok(cobj)
    }

    pub fn load_contract(
        &mut self,
        vmsta: &mut VMState,
        gas: &mut i64,
        addr: &ContractAddress,
    ) -> VmrtRes<Arc<ContractObj>> {
        self.resolve_contract(vmsta, gas, addr)
    }

    fn resolve_user_on_owner(
        &mut self,
        vmsta: &mut VMState,
        gas: &mut i64,
        owner: &ContractAddress,
        selector: FnSign,
    ) -> VmrtRes<Option<ResolvedFn>> {
        let csto = self.resolve_contract(vmsta, gas, owner)?;
        let Some(fnobj) = csto.userfns.get(&selector).cloned() else {
            return Ok(None);
        };
        Ok(Some(Self::build_resolved(owner, fnobj, csto.as_ref())))
    }

    fn resolve_abst_on_owner(
        &mut self,
        vmsta: &mut VMState,
        gas: &mut i64,
        owner: &ContractAddress,
        selector: AbstCall,
    ) -> VmrtRes<Option<ResolvedFn>> {
        let csto = self.resolve_contract(vmsta, gas, owner)?;
        let Some(fnobj) = csto.abstfns.get(&selector).cloned() else {
            return Ok(None);
        };
        Ok(Some(Self::build_resolved(owner, fnobj, csto.as_ref())))
    }

    pub fn resolve_abstfn(
        &mut self,
        ctx: &mut dyn Context,
        gas: &mut i64,
        addr: &ContractAddress,
        scty: AbstCall,
    ) -> VmrtRes<Option<ResolvedFn>> {
        let mut vmsta = VMState::wrap(ctx.state());
        if let Some(found) = self.resolve_abst_on_owner(&mut vmsta, gas, addr, scty)? {
            return Ok(Some(found));
        }
        let csto = self.resolve_contract(&mut vmsta, gas, addr)?;
        for parent in csto.sto.inherit.as_list() {
            if let Some(found) = self.resolve_abst_on_owner(&mut vmsta, gas, parent, scty)? {
                return Ok(Some(found));
            }
        }
        Ok(None)
    }

    fn resolve_lookup_candidates(
        &mut self,
        vmsta: &mut VMState,
        gas: &mut i64,
        anchor: &ContractAddress,
        call: &CallSpec,
    ) -> VmrtRes<Vec<ContractAddress>> {
        let parents = if call.needs_inherit_chain() {
            self.resolve_contract(vmsta, gas, anchor)?
                .sto
                .inherit
                .as_list()
                .to_vec()
        } else {
            vec![]
        };
        Ok(call.resolve_candidates(anchor, &parents))
    }

    fn resolve_user_call(
        &mut self,
        vmsta: &mut VMState,
        gas: &mut i64,
        call: &CallSpec,
        bindings: &FrameBindings,
    ) -> VmrtRes<(ContractAddress, ResolvedFn)> {
        let anchor = call.resolve_anchor(bindings)?;
        let entries = self.resolve_lookup_candidates(vmsta, gas, &anchor, call)?;
        let mut found = None;
        for owner in entries {
            if let Some(hit) = self.resolve_user_on_owner(vmsta, gas, &owner, call.selector())? {
                found = Some(hit);
                break;
            }
        }
        Ok((anchor, Self::require_resolved(found)?))
    }

    pub fn plan_user_call(
        &mut self,
        ctx: &mut dyn Context,
        gas: &mut i64,
        call: &CallSpec,
        bindings: &FrameBindings,
    ) -> VmrtRes<ResolvedCallPlan> {
        let mut vmsta = VMState::wrap(ctx.state());
        let (anchor, hit) = self.resolve_user_call(&mut vmsta, gas, call, bindings)?;
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
    use testkit::sim::state::FlatMemState as StateMem;

    fn test_contract(base: &Address, nonce: u32) -> ContractAddress {
        ContractAddress::calculate(base, &Uint4::from(nonce))
    }

    #[test]
    fn out_of_gas_on_cold_load_does_not_warm_tx_cache() {
        let base = Address::from_readable("1MzNY1oA3kfgYi75zquj3SRUPYztzXHzK9").unwrap();
        let caddr = test_contract(&base, 700_001);
        let csto = ContractSto::new();

        let mut state = StateMem::default();
        VMState::wrap(&mut state).contract_set_sync_edition(&caddr, &csto);

        let mut vmsta = VMState::wrap(&mut state);
        let mut res = Resoure::create(1);
        let mut gas_budget = 0i64;
        let err = match res.load_contract(&mut vmsta, &mut gas_budget, &caddr) {
            Ok(_) => panic!("expected OutOfGas"),
            Err(e) => e,
        };
        assert_eq!(err.0, ItrErrCode::OutOfGas);
        assert!(!res.contracts.contains_key(&caddr));
    }

    #[test]
    fn cold_load_charges_before_warming_and_hit_is_free() {
        let base = Address::from_readable("1MzNY1oA3kfgYi75zquj3SRUPYztzXHzK9").unwrap();
        let caddr = test_contract(&base, 700_002);
        let csto = ContractSto::new();
        let cbytes = csto.calc_edition().raw_size.uint() as usize;

        let mut state = StateMem::default();
        VMState::wrap(&mut state).contract_set_sync_edition(&caddr, &csto);

        let mut vmsta = VMState::wrap(&mut state);
        let mut res = Resoure::create(1);
        let one_cold_fee = res.gas_extra.load_new_contract + (cbytes as i64 / 64);
        let mut gas_budget = 10_000i64;

        {
            let _ = res
                .load_contract(&mut vmsta, &mut gas_budget, &caddr)
                .unwrap();
        }
        assert!(res.contracts.contains_key(&caddr));
        assert_eq!(gas_budget, 10_000 - one_cold_fee);

        let gas_after_first = gas_budget;
        {
            let _ = res
                .load_contract(&mut vmsta, &mut gas_budget, &caddr)
                .unwrap();
        }
        assert_eq!(gas_budget, gas_after_first);
    }
}
