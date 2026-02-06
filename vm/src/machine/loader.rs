/*

    contract loader

*/


impl Resoure {

    pub fn load_contract(&mut self, vmsta: &mut VMState, addr: &ContractAddress) -> VmrtRes<Arc<ContractObj>> {
        use ItrErrCode::*;
        if let Some(c) = self.contracts.get(addr) {
            return Ok(c.clone())
        }
        if self.contracts.len() >= self.space_cap.load_contract {
            return itr_err_code!(OutOfLoadContract)
        }
        let Some(c) = vmsta.contract(addr) else {
            return itr_err_fmt!(NotFindContract, "cannot find contract {}", addr.to_readable());
        };
        let rev = c.metas.revision.uint();
        let cbytes = c.size();
        if let Some(obj) = global_machine_manager()
            .contract_cache()
            .get(addr, rev)
        {
            self.contracts.insert(addr.clone(), obj.clone()); // tx-local cache
            self.contract_load_bytes = self.contract_load_bytes.saturating_add(cbytes);
            return Ok(obj);
        }
        let cobj = Arc::new(c.clone().into_obj()?);
        self.contracts.insert(addr.clone(), cobj.clone()); // tx-local cache
        global_machine_manager().contract_cache().insert(addr, &c, cobj.clone());
        self.contract_load_bytes = self.contract_load_bytes.saturating_add(cbytes);
        Ok(cobj)
    }


    fn load_fn_by_search_inherits(&mut self, vmsta: &mut VMState, addr: &ContractAddress, fnkey: FnKey) 
    -> VmrtRes<Option<(Option<ContractAddress>, Arc<FnObj>)>> {
        let mut visiting = std::collections::HashSet::new();
        let mut visited = std::collections::HashSet::new();
        let res = self.load_fn_by_search_inherits_rec(vmsta, addr, &fnkey, &mut visiting, &mut visited)?;
        Ok(res.map(|(owner, func)| {
            let change = maybe!(&owner == addr, None, Some(owner));
            (change, func)
        }))
    }

    fn load_fn_by_search_inherits_rec(
        &mut self,
        vmsta: &mut VMState,
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
        let csto = self.load_contract(vmsta, addr)?;
        let found = match fnkey {
            FnKey::Abst(s) => csto.abstfns.get(s),
            FnKey::User(u) => csto.userfns.get(u),
        };
        if let Some(c) = found {
            visiting.remove(addr);
            return Ok(Some((addr.clone(), c.clone())));
        }
        // DFS in inherits list order
        for ih in csto.sto.inherits.list() {
            if let Some(found) = self.load_fn_by_search_inherits_rec(vmsta, ih, fnkey, visiting, visited)? {
                visiting.remove(addr);
                return Ok(Some(found));
            }
        }
        visiting.remove(addr);
        visited.insert(addr.clone());
        Ok(None)
    }



    fn load_fn_by_search_librarys(
        &mut self, vmsta: &mut VMState, 
        srcadr: &ContractAddress, lib: u8, fnsg: FnSign
    ) -> VmrtRes<(ContractAddress, Option<Arc<FnObj>>)> {
        let csto = self.load_contract(vmsta, srcadr)?;
        let librarys = csto.sto.librarys.list();
        self.load_fn_by_search_list(vmsta, librarys, lib, fnsg)
    }

    fn load_fn_by_search_list(
        &mut self, vmsta: &mut VMState, adrlist: &Vec<ContractAddress>, lib: u8, fnsg: FnSign
    ) -> VmrtRes<(ContractAddress, Option<Arc<FnObj>>)> {
        use ItrErrCode::*;
        let librarys = adrlist;
        let libidx = lib as usize;
        if libidx >= librarys.len() {
            return itr_err_code!(CallViewOverflow)
        }
        let taradr = librarys.get(libidx).unwrap();
        taradr.check().map_ire(ContractAddrErr)?; // check must contract addr
        let csto = self.load_contract(vmsta, taradr)?;
        Ok((taradr.clone(), csto.userfns.get(&fnsg).map(|f|f.clone())))
    }

    pub fn load_userfn(&mut self, vmsta: &mut VMState, addr: &ContractAddress, fnsg: FnSign) -> VmrtRes<Option<(Option<ContractAddress>, Arc<FnObj>)>> {
        self.load_fn_by_search_inherits(vmsta, addr, FnKey::User(fnsg))
    }


    pub fn load_userfn_super(
        &mut self,
        vmsta: &mut VMState,
        curadr: &ContractAddress,
        fnsg: FnSign,
    ) -> VmrtRes<Option<(Option<ContractAddress>, Arc<FnObj>)>> {
        // Start from direct inherits of current code owner, skipping itself.
        let csto = self.load_contract(vmsta, curadr)?;
        let mut visiting = std::collections::HashSet::new();
        let mut visited = std::collections::HashSet::new();
        let fnkey = FnKey::User(fnsg);
        for ih in csto.sto.inherits.list() {
            if let Some((owner, func)) = self.load_fn_by_search_inherits_rec(vmsta, ih, &fnkey, &mut visiting, &mut visited)? {
                let change = maybe!(&owner == curadr, None, Some(owner));
                return Ok(Some((change, func)));
            }
        }
        Ok(None)
    }


    pub fn load_abstfn(&mut self, ctx: &mut dyn Context, addr: &ContractAddress, scty: AbstCall) -> VmrtRes<Option<(Option<ContractAddress>, Arc<FnObj>)>> {
        let mut vmsta = VMState::wrap(ctx.state());
        self.load_fn_by_search_inherits(&mut vmsta, addr, FnKey::Abst(scty))
    }

    /*
        return (change current address, fnobj)
    */
    pub fn load_must_call(&mut self, 
        ctx: &mut dyn Context, fptr: Funcptr, 
        dstadr: &ContractAddress, srcadr: &ContractAddress,
        adrlibs: &Option<Vec<ContractAddress>>
    ) -> VmrtRes<(Option<ContractAddress>, Arc<FnObj>)> {
        let mut vmsta = VMState::wrap(ctx.state());
        use CallTarget::*;
        use ItrErrCode::*;
        match match fptr.target {
            This => {
                let Some((a, b)) = self.load_userfn(&mut vmsta, dstadr, fptr.fnsign)? else {
                    return itr_err_code!(CallNotExist)
                };
                (a, Some(b))
            }
            Self_ => {
                let Some((a, b)) = self.load_userfn(&mut vmsta, srcadr, fptr.fnsign)? else {
                    return itr_err_code!(CallNotExist)
                };
                (a, Some(b))
            }
            Super => {
                let Some((a, b)) = self.load_userfn_super(&mut vmsta, srcadr, fptr.fnsign)? else {
                    return itr_err_code!(CallNotExist)
                };
                (a, Some(b))
            }
            // Addr(ctxadr)  => (Some(ctxadr.clone()), self.load_userfn(vmsta, &ctxadr, fptr.fnsign)?),
            Libidx(lib)   => match adrlibs {
                Some(ads) => self.load_fn_by_search_list(&mut vmsta, &ads, lib, fptr.fnsign),
                _ => self.load_fn_by_search_librarys(&mut vmsta, srcadr, lib, fptr.fnsign),
            }.map(|(a,b)|(Some(a), b))?,
        }  {
            (b, Some(c))  => Ok((b, c)),
            _ => itr_err_code!(CallNotExist), // 
        }
    }




}
