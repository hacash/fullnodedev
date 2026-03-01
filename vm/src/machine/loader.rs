/* contract loader */

#[derive(Debug, Clone, Copy)]
pub enum FnSelector {
    Abst(AbstCall),
    User(FnSign),
}

#[derive(Debug, Clone, Copy)]
pub enum LookupScope {
    RootOnly,
    RootThenDirectParents,
    DirectParentsOnly,
}

#[derive(Debug, Clone)]
pub struct ResolvedFn {
    pub owner: ContractAddress,
    pub fnobj: Arc<FnObj>,
}

#[derive(Debug, Clone)]
pub enum DispatchPlan {
    KeepState {
        code_owner: ContractAddress,
        fnobj: Arc<FnObj>,
    },
    SwitchState {
        state_addr: ContractAddress,
        code_owner: ContractAddress,
        fnobj: Arc<FnObj>,
    },
}

impl DispatchPlan {
    pub fn fnobj(&self) -> &Arc<FnObj> {
        match self {
            Self::KeepState { fnobj, .. } | Self::SwitchState { fnobj, .. } => fnobj,
        }
    }

    pub fn code_owner(&self) -> &ContractAddress {
        match self {
            Self::KeepState { code_owner, .. } | Self::SwitchState { code_owner, .. } => {
                code_owner
            }
        }
    }

    pub fn visibility_addr(&self) -> &ContractAddress {
        match self {
            Self::SwitchState { state_addr, .. } => state_addr,
            Self::KeepState { code_owner, .. } => code_owner,
        }
    }

    pub fn into_parts(self) -> (Option<ContractAddress>, ContractAddress, Arc<FnObj>) {
        match self {
            Self::KeepState { code_owner, fnobj } => (None, code_owner, fnobj),
            Self::SwitchState {
                state_addr,
                code_owner,
                fnobj,
            } => (Some(state_addr), code_owner, fnobj),
        }
    }
}

#[derive(Debug, Clone)]
pub struct CallPlanReq<'a> {
    pub fptr: Funcptr,
    pub state_addr: &'a ContractAddress,
    pub code_owner: &'a ContractAddress,
    pub tx_libs: &'a Option<Vec<ContractAddress>>,
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

        if self.contracts.len() >= self.space_cap.load_contract {
            return itr_err_code!(OutOfLoadContract);
        }

        let cbytes = state_ed.raw_size.uint() as usize;
        if let Some(obj) = global_machine_manager().contract_cache().get(addr, &state_ed) {
            self.settle_new_contract_load_gas(gas, cbytes)?;
            self.contracts.insert(addr.clone(), obj.clone());
            return Ok(obj);
        }

        let cobj = self.load_contract_from_state(vmsta, addr, &state_ed)?;
        self.settle_new_contract_load_gas(gas, cbytes)?;
        self.contracts.insert(addr.clone(), cobj.clone());
        global_machine_manager().contract_cache().insert(addr, cobj.clone());
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

    #[inline(always)]
    fn fn_lookup(csto: &ContractObj, selector: FnSelector) -> Option<Arc<FnObj>> {
        match selector {
            FnSelector::Abst(s) => csto.abstfns.get(&s).cloned(),
            FnSelector::User(u) => csto.userfns.get(&u).cloned(),
        }
    }

    fn resolve_on_owner(
        &mut self,
        vmsta: &mut VMState,
        gas: &mut i64,
        owner: &ContractAddress,
        selector: FnSelector,
    ) -> VmrtRes<Option<ResolvedFn>> {
        let csto = self.resolve_contract(vmsta, gas, owner)?;
        let Some(fnobj) = Self::fn_lookup(&csto, selector) else {
            return Ok(None);
        };
        Ok(Some(ResolvedFn {
            owner: owner.clone(),
            fnobj,
        }))
    }

    fn resolve_in_direct_parents(
        &mut self,
        vmsta: &mut VMState,
        gas: &mut i64,
        parents: &[ContractAddress],
        selector: FnSelector,
    ) -> VmrtRes<Option<ResolvedFn>> {
        for p in parents {
            if let Some(found) = self.resolve_on_owner(vmsta, gas, p, selector)? {
                return Ok(Some(found));
            }
        }
        Ok(None)
    }

    pub fn resolve_fn(
        &mut self,
        vmsta: &mut VMState,
        gas: &mut i64,
        owner: &ContractAddress,
        selector: FnSelector,
        scope: LookupScope,
    ) -> VmrtRes<Option<ResolvedFn>> {
        use LookupScope::*;
        let csto = self.resolve_contract(vmsta, gas, owner)?;
        match scope {
            RootOnly => Ok(Self::fn_lookup(&csto, selector).map(|fnobj| ResolvedFn {
                owner: owner.clone(),
                fnobj,
            })),
            RootThenDirectParents => {
                if let Some(fnobj) = Self::fn_lookup(&csto, selector) {
                    return Ok(Some(ResolvedFn {
                        owner: owner.clone(),
                        fnobj,
                    }));
                }
                self.resolve_in_direct_parents(vmsta, gas, csto.sto.inherits.as_list(), selector)
            }
            DirectParentsOnly => {
                self.resolve_in_direct_parents(vmsta, gas, csto.sto.inherits.as_list(), selector)
            }
        }
    }

    fn resolve_userfn(
        &mut self,
        vmsta: &mut VMState,
        gas: &mut i64,
        owner: &ContractAddress,
        scope: LookupScope,
        fnsg: FnSign,
    ) -> VmrtRes<Option<ResolvedFn>> {
        self.resolve_fn(vmsta, gas, owner, FnSelector::User(fnsg), scope)
    }

    pub fn resolve_abstfn(
        &mut self,
        ctx: &mut dyn Context,
        gas: &mut i64,
        addr: &ContractAddress,
        scty: AbstCall,
    ) -> VmrtRes<Option<ResolvedFn>> {
        let mut vmsta = VMState::wrap(ctx.state());
        self.resolve_fn(
            &mut vmsta,
            gas,
            addr,
            FnSelector::Abst(scty),
            LookupScope::RootThenDirectParents,
        )
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

    fn resolve_lib_addr_from_source(
        &mut self,
        vmsta: &mut VMState,
        gas: &mut i64,
        source: &ContractAddress,
        lib: u8,
    ) -> VmrtRes<ContractAddress> {
        let csto = self.load_contract(vmsta, gas, source)?;
        self.resolve_lib_addr_by_list(csto.sto.librarys.as_list(), lib)
    }

    pub fn plan_call(
        &mut self,
        ctx: &mut dyn Context,
        gas: &mut i64,
        req: CallPlanReq<'_>,
    ) -> VmrtRes<DispatchPlan> {
        let mut vmsta = VMState::wrap(ctx.state());
        use CallTarget::*;
        use ExecMode::*;
        match req.fptr.target {
            This => {
                let hit = Self::require_resolved(self.resolve_userfn(
                    &mut vmsta,
                    gas,
                    req.state_addr,
                    LookupScope::RootThenDirectParents,
                    req.fptr.fnsign,
                )?)?;
                Ok(DispatchPlan::KeepState {
                    code_owner: hit.owner,
                    fnobj: hit.fnobj,
                })
            }
            Self_ => {
                let hit = Self::require_resolved(self.resolve_userfn(
                    &mut vmsta,
                    gas,
                    req.code_owner,
                    LookupScope::RootThenDirectParents,
                    req.fptr.fnsign,
                )?)?;
                Ok(DispatchPlan::KeepState {
                    code_owner: hit.owner,
                    fnobj: hit.fnobj,
                })
            }
            Super => {
                let hit = Self::require_resolved(self.resolve_userfn(
                    &mut vmsta,
                    gas,
                    req.code_owner,
                    LookupScope::DirectParentsOnly,
                    req.fptr.fnsign,
                )?)?;
                Ok(DispatchPlan::KeepState {
                    code_owner: hit.owner,
                    fnobj: hit.fnobj,
                })
            }
            Idx(lib) => {
                let target_state = match req.tx_libs {
                    Some(ads) => self.resolve_lib_addr_by_list(ads, lib)?,
                    _ => self.resolve_lib_addr_from_source(&mut vmsta, gas, req.code_owner, lib)?,
                };
                if req.fptr.mode == Outer && !req.fptr.is_callcode {
                    let hit = Self::require_resolved(self.resolve_userfn(
                        &mut vmsta,
                        gas,
                        &target_state,
                        LookupScope::RootThenDirectParents,
                        req.fptr.fnsign,
                    )?)?;
                    return Ok(DispatchPlan::SwitchState {
                        state_addr: target_state,
                        code_owner: hit.owner,
                        fnobj: hit.fnobj,
                    });
                }
                let hit = Self::require_resolved(self.resolve_userfn(
                    &mut vmsta,
                    gas,
                    &target_state,
                    LookupScope::RootOnly,
                    req.fptr.fnsign,
                )?)?;
                Ok(DispatchPlan::KeepState {
                    code_owner: hit.owner,
                    fnobj: hit.fnobj,
                })
            }
        }
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
            let _ = res.load_contract(&mut vmsta, &mut gas_budget, &caddr).unwrap();
        }
        assert!(res.contracts.contains_key(&caddr));
        assert_eq!(gas_budget, 10_000 - one_cold_fee);

        let gas_after_first = gas_budget;
        {
            let _ = res.load_contract(&mut vmsta, &mut gas_budget, &caddr).unwrap();
        }
        assert_eq!(gas_budget, gas_after_first);
    }
}
