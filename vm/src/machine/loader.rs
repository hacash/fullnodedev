/* contract loader */

#[derive(Debug, Clone)]
pub struct CallLoad {
    pub state_addr: Option<ContractAddress>,
    pub code_owner: Option<ContractAddress>,
    pub fnobj: Arc<FnObj>,
}

impl Resoure {
    pub fn load_contract(
        &mut self,
        vmsta: &mut VMState,
        gas: &mut Option<&mut i64>,
        addr: &ContractAddress,
    ) -> VmrtRes<Arc<ContractObj>> {
        use ItrErrCode::*;
        if let Some(c) = self.contracts.get(addr) {
            return Ok(c.clone());
        }
        if self.contracts.len() >= self.space_cap.load_contract {
            return itr_err_code!(OutOfLoadContract);
        }
        let Some(rev) = vmsta.contractrev(addr) else {
            return itr_err_fmt!(
                NotFindContract,
                "cannot find contract revision {}",
                addr.to_readable()
            );
        };
        let rev = rev.uint();
        if let Some(obj) = global_machine_manager().contract_cache().get(addr, rev) {
            let cbytes = obj.sto.size();
            self.contracts.insert(addr.clone(), obj.clone()); // tx-local cache
            if let Some(g) = gas.as_deref_mut() {
                self.settle_new_contract_load_gas(g, cbytes)?;
            }
            return Ok(obj);
        }
        let Some(c) = vmsta.contract(addr) else {
            return itr_err_fmt!(
                NotFindContract,
                "cannot find contract {}",
                addr.to_readable()
            );
        };
        let cbytes = c.size();
        let cobj = Arc::new(c.into_obj()?);
        self.contracts.insert(addr.clone(), cobj.clone()); // tx-local cache
        global_machine_manager()
            .contract_cache()
            .insert(addr, &cobj.sto, cobj.clone());
        if let Some(g) = gas.as_deref_mut() {
            self.settle_new_contract_load_gas(g, cbytes)?;
        }
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
