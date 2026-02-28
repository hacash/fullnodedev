

// pub const CONTRACT_STORE_FEE_MUL: u64 = 50;
pub const CONTRACT_STORE_PERM_PERIODS: u64 = 10_000;


macro_rules! vmsto {
    ($ctx: expr) => {
        VMState::wrap($ctx.state())
    };
}



action_define!{ ContractDeploy, 40, 
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
        // check
        self.contract.check(hei)?;
        if self.contract.metas.revision.uint() != 0 {
            return errf!("contract revision must be 0 on deploy")
        }
        precheck_contract_store(&caddr, &self.contract, ctx)?;
        let accf  = AbstCall::Construct;
        let charge_bytes = self.contract.size();
        // spend protocol fee
        check_sub_contract_protocol_fee(ctx, &self.protocol_cost, charge_bytes)?;
        // save the contract
        vmsto!(ctx).contract_set_sync_edition(&caddr, &self.contract);
        // call the construct function
        let cargv = self.construct_argv.to_vec();
        if cargv.len() > SpaceCap::new(hei).max_value_size {
            return errf!("construct argv size overflow")
        }
        let hvaccf = contract_has_abst_call_by_inherits(ctx, &caddr, accf)?;
        if hvaccf { // have Construct func
            let cty = ExecMode::Abst as u8;
            let _ = setup_vm_run(
                ctx,
                cty,
                accf as u8,
                Arc::from(caddr.as_bytes()),
                Value::Bytes(cargv),
            )?;
        }
        // ok finish
        Ok(vec![])
    })
}






action_define!{ ContractUpdate, 41, 
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
        precheck_contract_store(&caddr, &new_contract, ctx)?;
        // spend protocol fee only when storage grows
        let old_size = contract.size();
        let new_size = new_contract.size();
        let delta_bytes = new_size.saturating_sub(old_size);
        check_sub_contract_protocol_fee(ctx, &self.protocol_cost, delta_bytes)?;
        let cty = ExecMode::Abst as u8;
        let sys = maybe!(did_change, Change, Append) as u8; // Change or Append
        let _ = setup_vm_run(
            ctx,
            cty,
            sys,
            Arc::from(caddr.as_bytes()),
            Value::Nil,
        )?;
        // save the new
        vmsto!(ctx).contract_set_sync_edition(&caddr, &new_contract);
        let caddr_real = caddr.to_addr();
        ctx.vm().invalidate_contract_cache(&caddr_real);
        Ok(vec![]) 
    })
}




/**************************************/



fn check_contract_self_reference(root_addr: &ContractAddress, root_contract: &ContractSto) -> Rerr {
    if root_contract.inherits.as_list().iter().any(|a| a == root_addr) {
        return errf!("contract cannot inherit itself {}", root_addr.to_readable())
    }
    if root_contract.librarys.as_list().iter().any(|a| a == root_addr) {
        return errf!("contract cannot link itself as library {}", root_addr.to_readable())
    }
    Ok(())
}

fn precheck_contract_links_and_calls(ctx: &mut dyn Context, root_addr: &ContractAddress, root_contract: &ContractSto) -> Rerr {
    let height = ctx.env().block.height;
    let mut vmsta = VMState::wrap(ctx.state());
    check_link_contracts_exist(&mut vmsta, root_addr, root_contract)?;
    check_inherits_acyclic(&mut vmsta, root_addr, root_contract)?;
    check_static_call_targets(&mut vmsta, root_addr, root_contract, height)?;
    Ok(())
}

fn precheck_contract_store(
    root_addr: &ContractAddress,
    root_contract: &ContractSto,
    ctx: &mut dyn Context,
) -> Rerr {
    check_contract_self_reference(root_addr, root_contract)?;
    precheck_contract_links_and_calls(ctx, root_addr, root_contract)
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
    for a in root_contract.librarys.as_list() {
        let _ = load_contract_for_check(vmsta, root_addr, root_contract, a, "library")?;
    }
    for a in root_contract.inherits.as_list() {
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
        for p in sto.inherits.as_list() {
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

#[derive(Clone, Copy)]
struct UserfnMeta {
    is_public: bool,
    param_count: usize,
}

fn contract_userfn_meta(contract: &ContractSto, sign: &FnSign) -> Option<UserfnMeta> {
    let f = contract
        .userfuncs
        .as_list()
        .iter()
        .find(|f| f.sign.to_array() == *sign)?;
    let pub_mark = FnConf::Public as u8;
    Some(UserfnMeta {
        is_public: f.fncnf[0] & pub_mark == pub_mark,
        param_count: f.pmdf.param_count(),
    })
}

fn resolve_userfn_meta_by_inherits(
    vmsta: &mut VMState,
    root_addr: &ContractAddress,
    root_contract: &ContractSto,
    addr: &ContractAddress,
    sign: &FnSign,
    visiting: &mut std::collections::HashSet<ContractAddress>,
    visited: &mut std::collections::HashSet<ContractAddress>,
) -> Ret<Option<(ContractAddress, UserfnMeta)>> {
    if visiting.contains(addr) {
        return errf!("inherits cyclic detected at {}", addr.to_readable())
    }
    if visited.contains(addr) {
        return Ok(None)
    }
    visiting.insert(addr.clone());
    let sto = load_contract_for_check(vmsta, root_addr, root_contract, addr, "inherits")?;
    if let Some(meta) = contract_userfn_meta(&sto, sign) {
        visiting.remove(addr);
        return Ok(Some((addr.clone(), meta)))
    }
    for p in sto.inherits.as_list() {
        if let Some(found) = resolve_userfn_meta_by_inherits(vmsta, root_addr, root_contract, p, sign, visiting, visited)? {
            visiting.remove(addr);
            return Ok(Some(found))
        }
    }
    visiting.remove(addr);
    visited.insert(addr.clone());
    Ok(None)
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
                    let libs = root_contract.librarys.as_list();
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
                    let Some((owner, meta)) = resolve_userfn_meta_by_inherits(
                        vmsta,
                        root_addr,
                        root_contract,
                        tar,
                        &sign,
                        &mut visiting,
                        &mut visited,
                    )? else {
                        return errf!(
                            "{}: call target {} function 0x{} not found in inherits",
                            func_tag,
                            tar.to_readable(),
                            hex::encode(sign)
                        )
                    };
                    if !meta.is_public {
                        return errf!(
                            "{}: call target {} function 0x{} resolved in {} is not public",
                            func_tag,
                            tar.to_readable(),
                            hex::encode(sign),
                            owner.to_readable()
                        );
                    }
                    Ok(())
                }
                Bytecode::CALLVIEW | Bytecode::CALLPURE => {
                    let libs = root_contract.librarys.as_list();
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
                    if contract_userfn_meta(&sto, &sign).is_none() {
                        return errf!(
                            "{}: library {} function 0x{} not found",
                            func_tag,
                            tar.to_readable(),
                            hex::encode(sign)
                        )
                    }
                    Ok(())
                }
                Bytecode::CALLCODE => {
                    let libs = root_contract.librarys.as_list();
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
                    let Some(meta) = contract_userfn_meta(&sto, &sign) else {
                        return errf!(
                            "{}: library {} function 0x{} not found",
                            func_tag,
                            tar.to_readable(),
                            hex::encode(sign)
                        )
                    };
                    if meta.param_count != 0 {
                        return errf!(
                            "{}: callcode target {} function 0x{} param_count {} must be 0",
                            func_tag,
                            tar.to_readable(),
                            hex::encode(sign),
                            meta.param_count
                        );
                    }
                    Ok(())
                }
                Bytecode::CALLTHIS | Bytecode::CALLSELF => {
                    // Deploy-time precheck validates callsites against the contract being deployed/updated. Runtime CALLSELF resolves from dynamic code_owner, which may differ after inherited dispatch. Cross-contract inherited bodies are validated in their own deploy/update.
                    sign.copy_from_slice(&params[..FN_SIGN_WIDTH]);
                    let mut visiting = std::collections::HashSet::new();
                    let mut visited = std::collections::HashSet::new();
                    let found = resolve_userfn_meta_by_inherits(
                        vmsta,
                        root_addr,
                        root_contract,
                        root_addr,
                        &sign,
                        &mut visiting,
                        &mut visited,
                    )?;
                    if found.is_none() {
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
                    for p in root_contract.inherits.as_list() {
                        let mut visiting = std::collections::HashSet::new();
                        let mut visited = std::collections::HashSet::new();
                        if resolve_userfn_meta_by_inherits(
                            vmsta,
                            root_addr,
                            root_contract,
                            p,
                            &sign,
                            &mut visiting,
                            &mut visited,
                        )?.is_some() {
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

    for f in root_contract.userfuncs.as_list() {
        let code_pkg = CodePkg::try_from(&f.code_stuff).map_err(|e| e.to_string())?;
        let ctype = code_pkg.code_type().map_err(|e| e.to_string())?;
        let codes = match ctype {
            CodeType::Bytecode => code_pkg.data,
            CodeType::IRNode => runtime_irs_to_bytecodes(&code_pkg.data, height).map_err(|e| e.to_string())?,
        };
        let tag = format!("userfn 0x{}", hex::encode(f.sign.to_array()));
        check_one(tag, &codes, vmsta)?;
    }

    for f in root_contract.abstcalls.as_list() {
        let code_pkg = CodePkg::try_from(&f.code_stuff).map_err(|e| e.to_string())?;
        let ctype = code_pkg.code_type().map_err(|e| e.to_string())?;
        let codes = match ctype {
            CodeType::Bytecode => code_pkg.data,
            CodeType::IRNode => runtime_irs_to_bytecodes(&code_pkg.data, height).map_err(|e| e.to_string())?,
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

fn contract_has_abst_call_by_inherits(
    ctx: &mut dyn Context,
    addr: &ContractAddress,
    call: AbstCall,
) -> Ret<bool> {
    let mut loader = Resoure::create(ctx.env().block.height);
    let found = loader
        .find_abstfn(ctx, addr, call)
        .map_err(|e| e.to_string())?;
    Ok(found.is_some())
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

#[cfg(test)]
mod contract_test {
    use super::*;
    use crate::contract::{Abst, Contract};
    use basis::component::Env;
    use basis::interface::{ActExec, Context};
    use field::{Address, Amount, Uint4};
    use protocol::context::decode_gas_budget;
    use protocol::transaction::TransactionType3;
    use std::sync::Once;
    use sys::IntoRet;
    use testkit::sim::context::make_ctx_with_state;
    use testkit::sim::state::FlatMemState as StateMem;
    use testkit::sim::tx::StubTxBuilder;

    fn test_main_addr() -> Address {
        Address::from_readable("1MzNY1oA3kfgYi75zquj3SRUPYztzXHzK9").unwrap()
    }

    fn test_contract(base: &Address, nonce: u32) -> ContractAddress {
        ContractAddress::calculate(base, &Uint4::from(nonce))
    }

    fn init_vm_assigner_once() {
        static INIT: Once = Once::new();
        INIT.call_once(|| {
            protocol::setup::vm_assigner(crate::machine::vm_assign);
        });
    }

    fn run_deploy_with_preloaded(
        nonce: u32,
        preload: Vec<(ContractAddress, ContractSto)>,
        deploy_contract: ContractSto,
    ) -> Ret<()> {
        init_vm_assigner_once();
        let main = test_main_addr();
        let tx = StubTxBuilder::new()
            .ty(TransactionType3::TYPE)
            .main(main)
            .addrs(vec![main])
            .fee(Amount::unit238(1_000_000))
            .gas_max(17)
            .tx_size(128)
            .fee_purity(1)
            .build();

        let mut env = Env::default();
        env.block.height = 1;
        env.chain.fast_sync = true; // skip action-level check in direct action tests
        env.tx.ty = tx.ty();
        env.tx.main = tx.main();
        env.tx.addrs = tx.addrs();

        let mut ext_state = StateMem::default();
        {
            let mut vmsta = VMState::wrap(&mut ext_state);
            for (addr, sto) in preload {
                vmsta.contract_set_sync_edition(&addr, &sto);
            }
        }

        let mut ctx = make_ctx_with_state(env, Box::new(ext_state), &tx);
        protocol::operate::hac_add(&mut ctx, &main, &Amount::unit238(10_000_000_000_000))?;
        ctx.gas_init_tx(decode_gas_budget(17), 1)?;

        let mut act = ContractDeploy::new();
        act.nonce = Uint4::from(nonce);
        act.protocol_cost = Amount::unit238(10_000_000);
        act.contract = deploy_contract;
        let _ = act.execute(&mut ctx).into_ret()?;
        Ok(())
    }

    #[test]
    fn deploy_construct_inherits_parent_when_local_absent() {
        let main = test_main_addr();
        let parent_addr = test_contract(&main, 31);
        let child_nonce = 32;

        let parent = Contract::new()
            .syst(
                Abst::new(AbstCall::Construct)
                    .fitsh("return 1")
                    .unwrap(),
            )
            .into_sto();
        let child = Contract::new().inh(parent_addr.to_addr()).into_sto();

        let err = run_deploy_with_preloaded(child_nonce, vec![(parent_addr, parent)], child)
            .expect_err("inherited parent Construct should execute and fail deploy");
        assert!(
            err.contains("Construct") && err.contains("return error code 1"),
            "unexpected deploy error: {err}"
        );
    }

    #[test]
    fn deploy_construct_prefers_local_over_parent() {
        let main = test_main_addr();
        let parent_addr = test_contract(&main, 33);
        let child_nonce = 34;

        let parent = Contract::new()
            .syst(
                Abst::new(AbstCall::Construct)
                    .fitsh("return 1")
                    .unwrap(),
            )
            .into_sto();
        let child = Contract::new()
            .inh(parent_addr.to_addr())
            .syst(
                Abst::new(AbstCall::Construct)
                    .fitsh("return 0")
                    .unwrap(),
            )
            .into_sto();

        let res = run_deploy_with_preloaded(child_nonce, vec![(parent_addr, parent)], child);
        assert!(res.is_ok(), "local Construct must override inherited one");
    }

    #[test]
    fn deploy_construct_searches_deep_inherits_chain() {
        let main = test_main_addr();
        let grand_addr = test_contract(&main, 35);
        let parent_addr = test_contract(&main, 36);
        let child_nonce = 37;

        let grand = Contract::new()
            .syst(
                Abst::new(AbstCall::Construct)
                    .fitsh("return 1")
                    .unwrap(),
            )
            .into_sto();
        let parent = Contract::new().inh(grand_addr.to_addr()).into_sto();
        let child = Contract::new().inh(parent_addr.to_addr()).into_sto();

        let err = run_deploy_with_preloaded(
            child_nonce,
            vec![(grand_addr, grand), (parent_addr, parent)],
            child,
        )
        .expect_err("deep inherited Construct should execute");
        assert!(
            err.contains("Construct") && err.contains("return error code 1"),
            "unexpected deploy error: {err}"
        );
    }

    #[test]
    fn deploy_without_any_construct_still_succeeds() {
        let main = test_main_addr();
        let parent_addr = test_contract(&main, 38);
        let child_nonce = 39;

        let parent = Contract::new().into_sto();
        let child = Contract::new().inh(parent_addr.to_addr()).into_sto();

        let res = run_deploy_with_preloaded(child_nonce, vec![(parent_addr, parent)], child);
        assert!(res.is_ok(), "deploy without Construct should keep old success path");
    }
}
