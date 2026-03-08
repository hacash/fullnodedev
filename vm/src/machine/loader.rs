/* contract loader */

#[derive(Debug, Clone)]
pub struct ResolvedFn {
    pub owner: ContractAddress,
    pub fnobj: Arc<FnObj>,
    pub lib_table: Arc<[ContractAddress]>,
}

#[derive(Debug, Clone)]
pub struct ResolvedCallPlan {
    pub next_bindings: FrameBindings,
    pub fnobj: Arc<FnObj>,
}

#[derive(Debug, Clone)]
pub struct CallPlanReq<'a> {
    pub call: &'a CallSpec,
    pub bindings: &'a FrameBindings,
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
            self.settle_new_contract_load_gas(gas, cbytes)?;
            self.contracts.insert(addr.clone(), obj.clone());
            return Ok(obj);
        }
        let cobj = self.load_contract_from_state(vmsta, addr, &state_ed)?;
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
        Ok(Some(ResolvedFn {
            owner: owner.clone(),
            fnobj,
            lib_table: csto.sto.library.as_list().to_vec().into(),
        }))
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
        Ok(Some(ResolvedFn {
            owner: owner.clone(),
            fnobj,
            lib_table: csto.sto.library.as_list().to_vec().into(),
        }))
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

    fn resolve_lib_addr_by_list(
        &self,
        adrlist: &[ContractAddress],
        lib: u8,
    ) -> VmrtRes<ContractAddress> {
        use ItrErrCode::*;
        let libidx = lib as usize;
        if libidx >= adrlist.len() {
            return itr_err_code!(CallLibIdxOverflow);
        }
        let taradr = adrlist.get(libidx).unwrap();
        taradr.check().map_ire(ContractAddrErr)?;
        Ok(taradr.clone())
    }

    fn resolve_lookup_anchor(&mut self, req: &CallPlanReq<'_>) -> VmrtRes<ContractAddress> {
        let target = req.call.target();
        if target.anchor_from_state() {
            return req
                .bindings
                .state_addr
                .clone()
                .ok_or_else(|| ItrErr::code(ItrErrCode::CallInvalid));
        }
        if target.anchor_from_code() {
            return req
                .bindings
                .code_owner
                .clone()
                .ok_or_else(|| ItrErr::code(ItrErrCode::CallInvalid));
        }
        self.resolve_lib_addr_by_list(req.bindings.lib_table.as_ref(), target.lib_index().unwrap())
    }

    fn resolve_lookup_candidates(
        &mut self,
        vmsta: &mut VMState,
        gas: &mut i64,
        anchor: &ContractAddress,
        call: &CallSpec,
    ) -> VmrtRes<Vec<ContractAddress>> {
        let target = call.target();
        if target.searches_exact() {
            return Ok(vec![anchor.clone()]);
        }
        if target.searches_parents() {
            return Ok(self
                .resolve_contract(vmsta, gas, anchor)?
                .sto
                .inherit
                .as_list()
                .iter()
                .cloned()
                .collect());
        }
        let csto = self.resolve_contract(vmsta, gas, anchor)?;
        let parents = csto.sto.inherit.as_list();
        let mut out = Vec::with_capacity(1 + parents.len());
        out.push(anchor.clone());
        out.extend(parents.iter().cloned());
        Ok(out)
    }

    fn resolve_user_call_fn(
        &mut self,
        vmsta: &mut VMState,
        gas: &mut i64,
        req: &CallPlanReq<'_>,
    ) -> VmrtRes<(ContractAddress, ResolvedFn)> {
        let anchor = self.resolve_lookup_anchor(req)?;
        let entries = self.resolve_lookup_candidates(vmsta, gas, &anchor, req.call)?;
        let mut found = None;
        for owner in entries {
            if let Some(hit) = self.resolve_user_on_owner(vmsta, gas, &owner, req.call.selector())? {
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
        req: CallPlanReq<'_>,
    ) -> VmrtRes<ResolvedCallPlan> {
        let mut vmsta = VMState::wrap(ctx.state());
        let (anchor, hit) = self.resolve_user_call_fn(&mut vmsta, gas, &req)?;
        if req.call.requires_external_visibility() && !hit.fnobj.check_conf(FnConf::External) {
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
                hex::encode(req.call.selector())
            );
        }
        let next_context_addr = if req.call.switches_context() {
            anchor.clone()
        } else {
            req.bindings.context_addr.clone()
        };
        let next_state_addr = if req.call.switches_context() {
            Some(anchor)
        } else {
            req.bindings.state_addr.clone()
        };
        Ok(ResolvedCallPlan {
            next_bindings: FrameBindings::new(
                next_context_addr,
                next_state_addr,
                Some(hit.owner),
                hit.lib_table,
            ),
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
