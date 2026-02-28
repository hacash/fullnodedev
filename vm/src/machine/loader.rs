/* contract loader */

#[derive(Debug, Clone)]
pub struct CallLoad {
    pub state_addr: Option<ContractAddress>,
    pub code_owner: Option<ContractAddress>,
    pub fnobj: Arc<FnObj>,
}

impl Resoure {
    fn charge_then_warm_tx_cache(
        &mut self,
        gas: &mut Option<&mut i64>,
        addr: &ContractAddress,
        obj: Arc<ContractObj>,
        cbytes: usize,
    ) -> VmrtRes<Arc<ContractObj>> {
        // No pay, no warm.
        if let Some(g) = gas.as_deref_mut() {
            self.settle_new_contract_load_gas(g, cbytes)?;
        }
        self.contracts.insert(addr.clone(), obj.clone());
        Ok(obj)
    }

    pub fn load_contract(
        &mut self,
        vmsta: &mut VMState,
        gas: &mut Option<&mut i64>,
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
            return self.charge_then_warm_tx_cache(gas, addr, obj, cbytes);
        }
        let Some(c) = vmsta.contract(addr) else {
            return itr_err_fmt!(
                NotFindContract,
                "cannot find contract {}",
                addr.to_readable()
            );
        };
        let cobj = Arc::new(c.into_obj()?);
        if cobj.edition != state_ed {
            return itr_err_fmt!(
                ContractError,
                "contract edition mismatch {}",
                addr.to_readable()
            );
        }
        let cobj = self.charge_then_warm_tx_cache(gas, addr, cobj, cbytes)?;
        global_machine_manager().contract_cache().insert(addr, cobj.clone());
        Ok(cobj)
    }

    fn load_fn_by_search_inherits(
        &mut self,
        vmsta: &mut VMState,
        gas: &mut Option<&mut i64>,
        addr: &ContractAddress,
        fnkey: FnKey,
    ) -> VmrtRes<Option<(Option<ContractAddress>, Arc<FnObj>)>> {
        let mut visiting = std::collections::HashSet::new();
        let mut visited = std::collections::HashSet::new();
        let res = self.load_fn_by_search_inherits_rec(
            vmsta,
            gas,
            addr,
            &fnkey,
            &mut visiting,
            &mut visited,
        )?;
        Ok(res.map(|(owner, func)| {
            let change = maybe!(&owner == addr, None, Some(owner));
            (change, func)
        }))
    }

    fn load_fn_by_search_inherits_rec(
        &mut self,
        vmsta: &mut VMState,
        gas: &mut Option<&mut i64>,
        addr: &ContractAddress,
        fnkey: &FnKey,
        visiting: &mut std::collections::HashSet<ContractAddress>,
        visited: &mut std::collections::HashSet<ContractAddress>,
    ) -> VmrtRes<Option<(ContractAddress, Arc<FnObj>)>> {
        if visiting.contains(addr) {
            return itr_err_fmt!(InheritsError, "inherits cyclic");
        }
        if visited.contains(addr) {
            return Ok(None);
        }
        visiting.insert(addr.clone());
        let csto = self.load_contract(vmsta, gas, addr)?;
        let found = match fnkey {
            FnKey::Abst(s) => csto.abstfns.get(s),
            FnKey::User(u) => csto.userfns.get(u),
        };
        if let Some(c) = found {
            visiting.remove(addr);
            return Ok(Some((addr.clone(), c.clone())));
        }
        // DFS in inherits list order
        for ih in csto.sto.inherits.as_list() {
            if let Some(found) =
                self.load_fn_by_search_inherits_rec(vmsta, gas, ih, fnkey, visiting, visited)?
            {
                visiting.remove(addr);
                return Ok(Some(found));
            }
        }
        visiting.remove(addr);
        visited.insert(addr.clone());
        Ok(None)
    }

    fn resolve_lib_addr_by_list(
        &self,
        adrlist: &Vec<ContractAddress>,
        lib: u8,
    ) -> VmrtRes<ContractAddress> {
        use ItrErrCode::*;
        let libidx = lib as usize;
        if libidx >= adrlist.len() {
            return itr_err_code!(CallLibIdxOverflow);
        }
        let taradr = adrlist.get(libidx).unwrap();
        taradr.check().map_ire(ContractAddrErr)?; // check must contract addr
        Ok(taradr.clone())
    }

    fn resolve_lib_addr_from_source(
        &mut self,
        vmsta: &mut VMState,
        gas: &mut Option<&mut i64>,
        source: &ContractAddress,
        lib: u8,
    ) -> VmrtRes<ContractAddress> {
        let csto = self.load_contract(vmsta, gas, source)?;
        self.resolve_lib_addr_by_list(csto.sto.librarys.as_list(), lib)
    }

    fn load_userfn(
        &mut self,
        vmsta: &mut VMState,
        gas: &mut Option<&mut i64>,
        addr: &ContractAddress,
        fnsg: FnSign,
    ) -> VmrtRes<Option<(Option<ContractAddress>, Arc<FnObj>)>> {
        self.load_fn_by_search_inherits(vmsta, gas, addr, FnKey::User(fnsg))
    }

    fn load_userfn_super(
        &mut self,
        vmsta: &mut VMState,
        gas: &mut Option<&mut i64>,
        code_owner: &ContractAddress,
        fnsg: FnSign,
    ) -> VmrtRes<Option<(Option<ContractAddress>, Arc<FnObj>)>> {
        // Start from direct inherits of current code owner, skipping itself.
        let csto = self.load_contract(vmsta, gas, code_owner)?;
        let mut visiting = std::collections::HashSet::new();
        let mut visited = std::collections::HashSet::new();
        // Keep super lookup from resolving back to current owner on malformed back-edge graphs.
        visited.insert(code_owner.clone());
        let fnkey = FnKey::User(fnsg);
        for ih in csto.sto.inherits.as_list() {
            if let Some((owner, func)) = self.load_fn_by_search_inherits_rec(
                vmsta,
                gas,
                ih,
                &fnkey,
                &mut visiting,
                &mut visited,
            )? {
                let change = maybe!(&owner == code_owner, None, Some(owner));
                return Ok(Some((change, func)));
            }
        }
        Ok(None)
    }

    pub fn load_abstfn(
        &mut self,
        ctx: &mut dyn Context,
        addr: &ContractAddress,
        scty: AbstCall,
    ) -> VmrtRes<Option<(Option<ContractAddress>, Arc<FnObj>)>> {
        let mut vmsta = VMState::wrap(ctx.state());
        let mut gas = None;
        self.load_fn_by_search_inherits(&mut vmsta, &mut gas, addr, FnKey::Abst(scty))
    }

    pub fn load_abstfn_with_gas(
        &mut self,
        ctx: &mut dyn Context,
        gas: &mut i64,
        addr: &ContractAddress,
        scty: AbstCall,
    ) -> VmrtRes<Option<(Option<ContractAddress>, Arc<FnObj>)>> {
        let mut vmsta = VMState::wrap(ctx.state());
        let mut gas = Some(gas);
        self.load_fn_by_search_inherits(&mut vmsta, &mut gas, addr, FnKey::Abst(scty))
    }

    /* return call target resolve result */
    pub fn load_must_call(
        &mut self,
        ctx: &mut dyn Context,
        gas: &mut i64,
        fptr: Funcptr,
        state_addr: &ContractAddress,
        code_owner: &ContractAddress,
        adrlibs: &Option<Vec<ContractAddress>>,
    ) -> VmrtRes<CallLoad> {
        let mut vmsta = VMState::wrap(ctx.state());
        let mut gas = Some(gas);
        use CallTarget::*;
        use ExecMode::*;
        use ItrErrCode::*;
        match fptr.target {
            This => {
                let Some((owner_change, fnobj)) =
                    self.load_userfn(&mut vmsta, &mut gas, state_addr, fptr.fnsign)?
                else {
                    return itr_err_code!(CallNotExist);
                };
                Ok(CallLoad {
                    state_addr: None,
                    code_owner: owner_change,
                    fnobj,
                })
            }
            Self_ => {
                let Some((owner_change, fnobj)) =
                    self.load_userfn(&mut vmsta, &mut gas, code_owner, fptr.fnsign)?
                else {
                    return itr_err_code!(CallNotExist);
                };
                Ok(CallLoad {
                    state_addr: None,
                    code_owner: owner_change,
                    fnobj,
                })
            }
            Super => {
                let Some((owner_change, fnobj)) =
                    self.load_userfn_super(&mut vmsta, &mut gas, code_owner, fptr.fnsign)?
                else {
                    return itr_err_code!(CallNotExist);
                };
                Ok(CallLoad {
                    state_addr: None,
                    code_owner: owner_change,
                    fnobj,
                })
            }
            // Addr(state_addr) => (Some(state_addr.clone()), self.load_userfn(vmsta, &state_addr, fptr.fnsign)?),
            Libidx(lib) => {
                let taradr = match adrlibs {
                    Some(ads) => self.resolve_lib_addr_by_list(ads, lib)?,
                    _ => {
                        self.resolve_lib_addr_from_source(&mut vmsta, &mut gas, code_owner, lib)?
                    }
                };
                // CALL (Outer) follows account semantics: function resolution includes inherits.
                if fptr.mode == Outer && !fptr.is_callcode {
                    let Some((owner_change, fnobj)) =
                        self.load_userfn(&mut vmsta, &mut gas, &taradr, fptr.fnsign)?
                    else {
                        return itr_err_code!(CallNotExist);
                    };
                    let owner = owner_change.unwrap_or_else(|| taradr.clone());
                    return Ok(CallLoad {
                        state_addr: Some(taradr),
                        code_owner: Some(owner),
                        fnobj,
                    });
                }
                // CALLVIEW/CALLPURE/CALLCODE keep library semantics: exact local lookup only.
                let csto = self.load_contract(&mut vmsta, &mut gas, &taradr)?;
                let Some(fnobj) = csto.userfns.get(&fptr.fnsign).cloned() else {
                    return itr_err_code!(CallNotExist);
                };
                Ok(CallLoad {
                    state_addr: None,
                    code_owner: Some(taradr),
                    fnobj,
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
        let mut gas = Some(&mut gas_budget);
        let err = match res.load_contract(&mut vmsta, &mut gas, &caddr) {
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
            let mut gas = Some(&mut gas_budget);
            let _ = res.load_contract(&mut vmsta, &mut gas, &caddr).unwrap();
        }
        assert!(res.contracts.contains_key(&caddr));
        assert_eq!(gas_budget, 10_000 - one_cold_fee);

        let gas_after_first = gas_budget;
        {
            let mut gas = Some(&mut gas_budget);
            let _ = res.load_contract(&mut vmsta, &mut gas, &caddr).unwrap();
        }
        assert_eq!(gas_budget, gas_after_first);
    }
}
