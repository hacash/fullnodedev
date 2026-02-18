

// pub const CONTRACT_STORE_FEE_MUL: u64 = 50;
pub const CONTRACT_STORE_PERM_PERIODS: u64 = 10_000;


macro_rules! vmsto {
    ($ctx: expr) => {
        VMState::wrap($ctx.state())
    };
}



action_define!{ContractDeploy, 99, 
    ActLv::TopOnlyWithGuard,
    false, [],
    {   
        protocol_cost: Amount
        nonce: Uint4 
        construct_argv: BytesW2 // checked by SpaceCap::max_value_size at runtime
        _marks_:   Fixed4 // zero
        contract: ContractSto
    },
    (self, {
        format!("Deploy smart contract with nonce {}", *self.nonce)
    }),
    (self, ctx, _gas {
        if self._marks_.not_zero() { // compatibility for future
            return errf!("marks byte error")
        }
        let hei = ctx.env().block.height;
        let maddr = ctx.env().tx.main;
        // check contract
        let caddr = ContractAddress::calculate(&maddr, &self.nonce);
        if vmsto!(ctx).contract_exist(&caddr) {
            return errf!("contract {} already exist", (*caddr).to_readable())
        }
        // cannot inherit self or link self as library
        if self.contract.inherits.list().iter().any(|a| a == &caddr) {
            return errf!("contract cannot inherit itself {}", (*caddr).to_readable())
        }
        if self.contract.librarys.list().iter().any(|a| a == &caddr) {
            return errf!("contract cannot link itself as library {}", (*caddr).to_readable())
        }
        // check
        self.contract.check(hei)?;
        if self.contract.metas.revision.uint() != 0 {
            return errf!("contract revision must be 0 on deploy")
        }
        precheck_contract_links_and_calls(ctx, &caddr, &self.contract)?;
        let accf  = AbstCall::Construct;
        let hvaccf = self.contract.have_abst_call(accf);
        let charge_bytes = self.contract.size();
        // spend protocol fee
        check_sub_contract_protocol_fee(ctx, &self.protocol_cost, charge_bytes)?;
        // save the contract
        vmsto!(ctx).contract_set(&caddr, &self.contract);
        // call the construct function
        let cargv = self.construct_argv.to_vec();
        if cargv.len() > SpaceCap::new(hei).max_value_size {
            return errf!("construct argv size overflow")
        }
        if hvaccf { // have Construct func
            let cty = ExecMode::Abst as u8;
            setup_vm_run(ctx, cty, accf as u8, Arc::from(caddr.as_bytes()), Value::Bytes(cargv))?;
        }
        // ok finish
        Ok(vec![])
    })
}






action_define!{ContractUpdate, 98, 
    ActLv::TopOnlyWithGuard, // level
    false, [], // burn 90% fee
    {   
        protocol_cost: Amount
        address: Address // contract address
        _marks_: Fixed2 // zero
        edit: ContractEdit
    },
    (self, format!("Update smart contract {}", self.address)),
    (self, ctx, _gas {
        use AbstCall::*;
        if self._marks_.not_zero() {
            return errf!("marks byte error")
        }
        let hei = ctx.env().block.height;
        // load old
        let caddr = ContractAddress::from_addr(self.address)?;
        let Some(contract) = vmsto!(ctx).contract(&caddr) else {
            return errf!("contract {} not exist", (*caddr).to_readable())
        };
        // apply edit (in memory)
		let mut new_contract = contract.clone();
        let (_did_append, did_change) = new_contract.apply_edit(&self.edit, hei)?;
        // cannot inherit self or link self as library
        if new_contract.inherits.list().iter().any(|a| a == &caddr) {
            return errf!("contract cannot inherit itself {}", (*caddr).to_readable())
        }
        if new_contract.librarys.list().iter().any(|a| a == &caddr) {
            return errf!("contract cannot link itself as library {}", (*caddr).to_readable())
        }
        precheck_contract_links_and_calls(ctx, &caddr, &new_contract)?;
        // spend protocol fee only when storage grows
        let old_size = contract.size();
        let new_size = new_contract.size();
        let delta_bytes = new_size.saturating_sub(old_size);
        check_sub_contract_protocol_fee(ctx, &self.protocol_cost, delta_bytes)?;
        let cty = ExecMode::Abst as u8;
        let sys = maybe!(did_change, Change, Append) as u8; // Change or Append
        setup_vm_run(ctx, cty, sys, Arc::from(caddr.as_bytes()), Value::Nil)?;
        // save the new
        vmsto!(ctx).contract_set(&caddr, &new_contract);
        Ok(vec![]) 
    })
}




/**************************************/



fn precheck_contract_links_and_calls(
    ctx: &mut dyn Context,
    root_addr: &ContractAddress,
    root_contract: &ContractSto,
) -> Rerr {
    let height = ctx.env().block.height;
    let mut vmsta = VMState::wrap(ctx.state());
    check_link_contracts_exist(&mut vmsta, root_addr, root_contract)?;
    check_inherits_acyclic(&mut vmsta, root_addr, root_contract)?;
    check_static_call_targets(&mut vmsta, root_addr, root_contract, height)?;
    Ok(())
}

fn load_contract_for_check(
    vmsta: &mut VMState,
    root_addr: &ContractAddress,
    root_contract: &ContractSto,
    addr: &ContractAddress,
    role: &str,
) -> Ret<ContractSto> {
    if addr == root_addr {
        return Ok(root_contract.clone())
    }
    match vmsta.contract(addr) {
        Some(c) => Ok(c),
        None => errf!("{} contract {} not exist", role, addr.to_readable()),
    }
}

fn check_link_contracts_exist(
    vmsta: &mut VMState,
    root_addr: &ContractAddress,
    root_contract: &ContractSto,
) -> Rerr {
    for a in root_contract.librarys.list() {
        let _ = load_contract_for_check(vmsta, root_addr, root_contract, a, "library")?;
    }
    for a in root_contract.inherits.list() {
        let _ = load_contract_for_check(vmsta, root_addr, root_contract, a, "inherits")?;
    }
    Ok(())
}

fn check_inherits_acyclic(
    vmsta: &mut VMState,
    root_addr: &ContractAddress,
    root_contract: &ContractSto,
) -> Rerr {
    fn dfs(
        vmsta: &mut VMState,
        root_addr: &ContractAddress,
        root_contract: &ContractSto,
        addr: &ContractAddress,
        visiting: &mut std::collections::HashSet<ContractAddress>,
        visited: &mut std::collections::HashSet<ContractAddress>,
    ) -> Rerr {
        if visiting.contains(addr) {
            return errf!("inherits cyclic detected at {}", addr.to_readable())
        }
        if visited.contains(addr) {
            return Ok(())
        }
        visiting.insert(addr.clone());
        let sto = load_contract_for_check(vmsta, root_addr, root_contract, addr, "inherits")?;
        for p in sto.inherits.list() {
            dfs(vmsta, root_addr, root_contract, p, visiting, visited)?;
        }
        visiting.remove(addr);
        visited.insert(addr.clone());
        Ok(())
    }

    let mut visiting = std::collections::HashSet::new();
    let mut visited = std::collections::HashSet::new();
    dfs(vmsta, root_addr, root_contract, root_addr, &mut visiting, &mut visited)
}

fn contract_has_userfn(contract: &ContractSto, sign: &FnSign) -> bool {
    contract
        .userfuncs
        .list()
        .iter()
        .any(|f| f.sign.to_array() == *sign)
}

fn resolve_userfn_by_inherits(
    vmsta: &mut VMState,
    root_addr: &ContractAddress,
    root_contract: &ContractSto,
    addr: &ContractAddress,
    sign: &FnSign,
    visiting: &mut std::collections::HashSet<ContractAddress>,
    visited: &mut std::collections::HashSet<ContractAddress>,
) -> Ret<bool> {
    if visiting.contains(addr) {
        return errf!("inherits cyclic detected at {}", addr.to_readable())
    }
    if visited.contains(addr) {
        return Ok(false)
    }
    visiting.insert(addr.clone());
    let sto = load_contract_for_check(vmsta, root_addr, root_contract, addr, "inherits")?;
    if contract_has_userfn(&sto, sign) {
        visiting.remove(addr);
        return Ok(true)
    }
    for p in sto.inherits.list() {
        if resolve_userfn_by_inherits(vmsta, root_addr, root_contract, p, sign, visiting, visited)? {
            visiting.remove(addr);
            return Ok(true)
        }
    }
    visiting.remove(addr);
    visited.insert(addr.clone());
    Ok(false)
}

fn scan_call_sites(codes: &[u8], mut check: impl FnMut(Bytecode, &[u8]) -> Rerr) -> Rerr {
    let mut i = 0usize;
    while i < codes.len() {
        let inst: Bytecode = std_mem_transmute!(codes[i]);
        let meta = inst.metadata();
        if !meta.valid {
            return errf!("invalid bytecode {}", codes[i])
        }
        i += 1;
        let pms = meta.param as usize;
        if i + pms > codes.len() {
            return errf!("instruction param overflow at {}", i - 1)
        }
        let params = &codes[i..i + pms];
        match inst {
            Bytecode::CALL
            | Bytecode::CALLTHIS
            | Bytecode::CALLSELF
            | Bytecode::CALLSUPER
            | Bytecode::CALLVIEW
            | Bytecode::CALLPURE
            | Bytecode::CALLCODE => {
                check(inst, params)?;
            }
            Bytecode::PBUF => {
                let l = params[0] as usize;
                if i + pms + l > codes.len() {
                    return errf!("PBUF overflow at {}", i - 1)
                }
                i += l;
            }
            Bytecode::PBUFL => {
                let l = u16::from_be_bytes([params[0], params[1]]) as usize;
                if i + pms + l > codes.len() {
                    return errf!("PBUFL overflow at {}", i - 1)
                }
                i += l;
            }
            _ => {}
        }
        i += pms;
    }
    Ok(())
}

fn check_static_call_targets(
    vmsta: &mut VMState,
    root_addr: &ContractAddress,
    root_contract: &ContractSto,
    height: u64,
) -> Rerr {
    let check_one = |func_tag: String, codes: &[u8], vmsta: &mut VMState| -> Rerr {
        scan_call_sites(codes, |inst, params| {
            let mut sign = [0u8; FN_SIGN_WIDTH];
            match inst {
                Bytecode::CALL => {
                    let libs = root_contract.librarys.list();
                    let libidx = params[0] as usize;
                    if libidx >= libs.len() {
                        return errf!(
                            "{}: libidx overflow {} >= {}",
                            func_tag,
                            libidx,
                            libs.len()
                        )
                    }
                    sign.copy_from_slice(&params[1..1 + FN_SIGN_WIDTH]);
                    let tar = &libs[libidx];
                    let mut visiting = std::collections::HashSet::new();
                    let mut visited = std::collections::HashSet::new();
                    let found = resolve_userfn_by_inherits(
                        vmsta,
                        root_addr,
                        root_contract,
                        tar,
                        &sign,
                        &mut visiting,
                        &mut visited,
                    )?;
                    if !found {
                        return errf!(
                            "{}: call target {} function 0x{} not found in inherits",
                            func_tag,
                            tar.to_readable(),
                            hex::encode(sign)
                        )
                    }
                    Ok(())
                }
                Bytecode::CALLVIEW | Bytecode::CALLPURE | Bytecode::CALLCODE => {
                    let libs = root_contract.librarys.list();
                    let libidx = params[0] as usize;
                    if libidx >= libs.len() {
                        return errf!(
                            "{}: libidx overflow {} >= {}",
                            func_tag,
                            libidx,
                            libs.len()
                        )
                    }
                    sign.copy_from_slice(&params[1..1 + FN_SIGN_WIDTH]);
                    let tar = &libs[libidx];
                    let sto =
                        load_contract_for_check(vmsta, root_addr, root_contract, tar, "library")?;
                    if !contract_has_userfn(&sto, &sign) {
                        return errf!(
                            "{}: library {} function 0x{} not found",
                            func_tag,
                            tar.to_readable(),
                            hex::encode(sign)
                        )
                    }
                    Ok(())
                }
                Bytecode::CALLTHIS | Bytecode::CALLSELF => {
                    // Deploy-time precheck validates callsites against the contract being deployed/updated. Runtime CALLSELF resolves from dynamic code_owner, which may differ after inherited dispatch. Cross-contract inherited bodies are validated in their own deploy/update.
                    sign.copy_from_slice(&params[..FN_SIGN_WIDTH]);
                    let mut visiting = std::collections::HashSet::new();
                    let mut visited = std::collections::HashSet::new();
                    let found = resolve_userfn_by_inherits(
                        vmsta,
                        root_addr,
                        root_contract,
                        root_addr,
                        &sign,
                        &mut visiting,
                        &mut visited,
                    )?;
                    if !found {
                        return errf!(
                            "{}: {:?} function 0x{} not found",
                            func_tag,
                            inst,
                            hex::encode(sign)
                        )
                    }
                    Ok(())
                }
                Bytecode::CALLSUPER => {
                    sign.copy_from_slice(&params[..FN_SIGN_WIDTH]);
                    let mut found = false;
                    for p in root_contract.inherits.list() {
                        let mut visiting = std::collections::HashSet::new();
                        let mut visited = std::collections::HashSet::new();
                        if resolve_userfn_by_inherits(
                            vmsta,
                            root_addr,
                            root_contract,
                            p,
                            &sign,
                            &mut visiting,
                            &mut visited,
                        )? {
                            found = true;
                            break;
                        }
                    }
                    if !found {
                        return errf!("{}: super function 0x{} not found", func_tag, hex::encode(sign))
                    }
                    Ok(())
                }
                _ => Ok(()),
            }
        })
    };

    for f in root_contract.userfuncs.list() {
        let ctype = CodeType::parse(f.cdty[0]).map_err(|e| e.to_string())?;
        let codes = match ctype {
            CodeType::Bytecode => f.code.to_vec(),
            CodeType::IRNode => runtime_irs_to_bytecodes(&f.code, height).map_err(|e| e.to_string())?,
        };
        let tag = format!("userfn 0x{}", hex::encode(f.sign.to_array()));
        check_one(tag, &codes, vmsta)?;
    }

    for f in root_contract.abstcalls.list() {
        let ctype = CodeType::parse(f.cdty[0]).map_err(|e| e.to_string())?;
        let codes = match ctype {
            CodeType::Bytecode => f.code.to_vec(),
            CodeType::IRNode => runtime_irs_to_bytecodes(&f.code, height).map_err(|e| e.to_string())?,
        };
        let tag = format!("abstcall {}", f.sign[0]);
        check_one(tag, &codes, vmsta)?;
    }

    Ok(())
}

fn check_sub_contract_protocol_fee(ctx: &mut dyn Context, pfee: &Amount, charge_bytes: usize) -> Rerr {
    if pfee.is_negative() {
		return errf!("protocol fee cannot be negative")
    }
    if charge_bytes == 0 {
        return Ok(())
    }
    let min_fee = calc_contract_protocol_fee_min(ctx, charge_bytes)?;
    let maddr = ctx.env().tx.main;
    // check fee
    if pfee < &min_fee { 
        return errf!(
            "protocol fee must need at least {} (bytes={}, periods={}) but just got {}",
            &min_fee,
            charge_bytes,
            contract_store_perm_periods(ctx.env().block.height),
            pfee
        )
    }
    operate::hac_sub(ctx, &maddr, pfee)?;
    Ok(())
}

#[inline(always)]
fn contract_store_perm_periods(_hei: u64) -> u64 {
    // Keep this as a function to make future fork-by-height tuning low-coupling.
    CONTRACT_STORE_PERM_PERIODS
}

fn calc_contract_protocol_fee_min(ctx: &dyn Context, charge_bytes: usize) -> Ret<Amount> {
    if charge_bytes == 0 {
        return Ok(Amount::zero())
    }
    let periods = contract_store_perm_periods(ctx.env().block.height) as u128;
    let fee_purity = ctx.tx().fee_purity() as u128; // unit-238 per tx byte
    if periods == 0 || fee_purity == 0 {
        return errf!(
            "contract protocol fee calculate failed: periods={} fee_purity={}",
            periods,
            fee_purity
        )
    }
    let bytes = charge_bytes as u128;
    let Some(need) = fee_purity.checked_mul(bytes) else {
        return errf!(
            "contract protocol fee calculate failed: fee_purity * bytes overflow ({} * {})",
            fee_purity,
            bytes
        )
    };
    let Some(need) = need.checked_mul(periods) else {
        return errf!(
            "contract protocol fee calculate failed: need * periods overflow ({} * {})",
            need,
            periods
        )
    };
    Ok(Amount::coin_u128(need, UNIT_238))
}




/* ************************************* fn check_sub_contract_protocol_fee(ctx: &mut dyn Context, ctlsz: usize, ptcfee: &Amount) -> Rerr { // let _hei = ctx.env().block.height; let e = errf!("contract protocol fee calculate failed"); let mul = CONTRACT_STORE_FEE_MUL as u128; // 30 let feep = ctx.tx().fee_purity() as u128; // per-byte, no GSCU division let Some(rlfe) = feep.checked_mul(ctlsz as u128) else { return e }; let Some(rlfe) = rlfe.checked_mul(mul) else { return e }; let tx50fee = &Amount::coin_u128(rlfe, UNIT_238).compress(2, AmtCpr::Grow)?; if tx50fee <= ctx.tx().fee() { return e } println!("{}, {}, {}, {}", ctx.tx().size(), ctlsz, ctx.tx().fee(), tx50fee); let maddr = ctx.env().tx.main; // check fee if ptcfee < tx50fee { return errf!("protocol fee must need at least {} but just got {}", tx50fee, ptcfee) } operate::hac_sub(ctx, &maddr, ptcfee)?; Ok(()) } */
