// pub const CONTRACT_STORE_FEE_MUL: u64 = 50;
pub const CONTRACT_STORE_PERM_PERIODS: u64 = 10_000;

macro_rules! vmsto {
    ($ctx: expr) => {
        VMState::wrap($ctx.state())
    };
}

action_define! { ContractDeploy, 40,
    ActScope::TOP_ONLY_WITH_GUARD, 3, false, [],
    {
        protocol_cost: Amount
        nonce: Uint4
        construct_call: Bool
        construct_argv: BytesW2 // checked by SpaceCap::value_size at runtime
        _marks_:   Fixed4 // zero
        contract: ContractSto
    },
    (self, {
        format!("Deploy smart contract with nonce {}", *self.nonce)
    }),
    (self, ctx, _gas {
        if self._marks_.not_zero() { // compatibility for future
            return errf!("marks bytes invalid")
        }
        let hei = ctx.env().block.height;
        let maddr = ctx.env().tx.main;
        // check contract
        let caddr = ContractAddress::calculate(&maddr, &self.nonce);
        if vmsto!(ctx).contract_exist(&caddr) {
            return errf!("contract {} already exists", (*caddr).to_readable())
        }
        // check
        self.contract.check(hei)?;
        if self.contract.metas.revision.uint() != 0 {
            return errf!("contract revision must be 0 on deploy")
        }
        precheck_contract_store(&caddr, &self.contract, ctx)?;
        let charge_bytes = self.contract.size();
        // spend protocol fee
        check_sub_contract_protocol_fee(ctx, &self.protocol_cost, charge_bytes)?;
        // save the contract
        vmsto!(ctx).contract_set_sync_edition(&caddr, &self.contract);
        let cargv = self.construct_argv.to_vec();
        if cargv.len() > SpaceCap::new(hei).value_size {
            return errf!("construct argv size overflow")
        }
        // call construct only when explicitly enabled by action flag
        if self.construct_call.check() {
            let _ = setup_vm_run_abst(
                ctx,
                AbstCall::Construct,
                Arc::from(caddr.as_bytes()),
                Value::Bytes(cargv),
            )?;
        }
        // ok finish
        Ok(vec![])
    })
}

action_define! { ContractUpdate, 41,
    ActScope::TOP_ONLY_WITH_GUARD, 3, false, [],
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
            return errf!("marks bytes invalid")
        }
        let hei = ctx.env().block.height;
        // load old
        let caddr = ContractAddress::from_addr(self.address)?;
        let Some(contract) = vmsto!(ctx).contract(&caddr) else {
            return errf!("contract {} does not exist", (*caddr).to_readable())
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
        // Design choice: `Change` intentionally dominates `Append`; any edit marked as changed (including abstcall edits) dispatches `Change`, and `Append` is reserved for append-only growth.
        let sys = maybe!(did_change, Change, Append); // Change or Append
        // Run Change/Append hook on the current on-chain contract; commit the updated contract only after hook success.
        let _ = setup_vm_run_abst(
            ctx,
            sys,
            Arc::from(caddr.as_bytes()),
            Value::Nil,
        )?;
        // save the new
        vmsto!(ctx).contract_set_sync_edition(&caddr, &new_contract);
        let caddr_real = caddr.to_addr();
        ctx.vm_invalidate_contract_cache(&caddr_real);
        Ok(vec![])
    })
}

/**************************************/

fn check_contract_self_reference(root_addr: &ContractAddress, root_contract: &ContractSto) -> Rerr {
    if root_contract
        .inherit
        .as_list()
        .iter()
        .any(|a| a == root_addr)
    {
        return errf!("contract cannot inherit itself {}", root_addr.to_readable());
    }
    if root_contract
        .library
        .as_list()
        .iter()
        .any(|a| a == root_addr)
    {
        return errf!(
            "contract cannot link itself as library {}",
            root_addr.to_readable()
        );
    }
    Ok(())
}

fn precheck_contract_links_and_calls(
    ctx: &mut dyn Context,
    root_addr: &ContractAddress,
    root_contract: &ContractSto,
) -> Rerr {
    let height = ctx.env().block.height;
    let mut vmsta = VMState::wrap(ctx.state());
    check_link_contracts_exist(&mut vmsta, root_addr, root_contract)?;
    check_inherits_direct_parents_flat(&mut vmsta, root_addr, root_contract)?;
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
        return Ok(root_contract.clone());
    }
    match vmsta.contract(addr) {
        Some(c) => Ok(c),
        None => errf!("{} contract {} does not exist", role, addr.to_readable()),
    }
}

fn check_link_contracts_exist(
    vmsta: &mut VMState,
    root_addr: &ContractAddress,
    root_contract: &ContractSto,
) -> Rerr {
    for a in root_contract.library.as_list() {
        let _ = load_contract_for_check(vmsta, root_addr, root_contract, a, "library")?;
    }
    for a in root_contract.inherit.as_list() {
        let _ = load_contract_for_check(vmsta, root_addr, root_contract, a, "inherit")?;
    }
    Ok(())
}

fn check_inherits_direct_parents_flat(
    vmsta: &mut VMState,
    root_addr: &ContractAddress,
    root_contract: &ContractSto,
) -> Rerr {
    for p in root_contract.inherit.as_list() {
        let sto = load_contract_for_check(vmsta, root_addr, root_contract, p, "inherit")?;
        if sto.inherit.length() > 0 {
            return errf!(
                "inherit parent {} cannot have parent inherit",
                p.to_readable()
            );
        }
    }
    Ok(())
}

#[derive(Clone, Copy)]
struct UserfnMeta {
    is_external: bool,
}

fn contract_userfn_meta(contract: &ContractSto, sign: &FnSign) -> Option<UserfnMeta> {
    let f = contract
        .userfuncs
        .as_list()
        .iter()
        .find(|f| f.sign.to_array() == *sign)?;
    let ext_mark = FnConf::External as u8;
    Some(UserfnMeta {
        is_external: f.fncnf[0] & ext_mark == ext_mark,
    })
}

fn scan_call_sites(codes: &[u8], mut check: impl FnMut(Bytecode, &[u8]) -> Rerr) -> Rerr {
    let mut i = 0usize;
    while i < codes.len() {
        let inst: Bytecode = std_mem_transmute!(codes[i]);
        let meta = inst.metadata();
        if !meta.valid {
            return errf!("invalid bytecode {}", codes[i]);
        }
        i += 1;
        let pms = meta.param as usize;
        if i + pms > codes.len() {
            return errf!("instruction param overflow at {}", i - 1);
        }
        let params = &codes[i..i + pms];
        match inst {
            _ if is_user_call_inst(inst) => {
                check(inst, params)?;
            }
            Bytecode::PBUF => {
                let l = params[0] as usize;
                if i + pms + l > codes.len() {
                    return errf!("PBUF overflow at {}", i - 1);
                }
                i += l;
            }
            Bytecode::PBUFL => {
                let l = u16::from_be_bytes([params[0], params[1]]) as usize;
                if i + pms + l > codes.len() {
                    return errf!("PBUFL overflow at {}", i - 1);
                }
                i += l;
            }
            _ => {}
        }
        i += pms;
    }
    Ok(())
}

fn resolve_userfn_meta_on_owner(
    vmsta: &mut VMState,
    root_addr: &ContractAddress,
    root_contract: &ContractSto,
    owner: &ContractAddress,
    sign: &FnSign,
) -> Ret<Option<(ContractAddress, UserfnMeta)>> {
    let sto = load_contract_for_check(vmsta, root_addr, root_contract, owner, "lookup")?;
    Ok(contract_userfn_meta(&sto, sign).map(|meta| (owner.clone(), meta)))
}

fn resolve_lookup_anchor_for_check(
    vmsta: &mut VMState,
    root_addr: &ContractAddress,
    root_contract: &ContractSto,
    func_tag: &str,
    call: &CallSpec,
) -> Ret<ContractAddress> {
    let lib_addrs: Vec<Address> = root_contract
        .library
        .as_list()
        .iter()
        .map(|a| a.to_addr())
        .collect();
    // Static precheck binds `this` to the contract being stored, so `this.*` must not be purely virtual: a default implementation must already exist on self or an inherited parent.
    let anchor = call
        .resolve_anchor_from(Some(root_addr), Some(root_addr), &lib_addrs)
        .map_err(|e| format!("{}: {}", func_tag, e))?;
    if call.lib_index().is_some() {
        let _ = load_contract_for_check(vmsta, root_addr, root_contract, &anchor, "lookup")?;
    }
    Ok(anchor)
}

fn resolve_lookup_entries_for_check(
    vmsta: &mut VMState,
    root_addr: &ContractAddress,
    root_contract: &ContractSto,
    anchor: &ContractAddress,
    call: &CallSpec,
) -> Ret<Vec<ContractAddress>> {
    let parents = if call.needs_inherit_chain() {
        load_contract_for_check(vmsta, root_addr, root_contract, anchor, "inherit")?
            .inherit
            .as_list()
            .to_vec()
    } else {
        vec![]
    };
    Ok(call.resolve_candidates(anchor, &parents))
}

fn resolve_userfn_meta_by_lookup_for_check(
    vmsta: &mut VMState,
    root_addr: &ContractAddress,
    root_contract: &ContractSto,
    func_tag: &str,
    call: &CallSpec,
    sign: &FnSign,
) -> Ret<Option<(ContractAddress, UserfnMeta)>> {
    let anchor = resolve_lookup_anchor_for_check(vmsta, root_addr, root_contract, func_tag, call)?;
    let entries = resolve_lookup_entries_for_check(vmsta, root_addr, root_contract, &anchor, call)?;
    for owner in entries {
        if let Some(hit) =
            resolve_userfn_meta_on_owner(vmsta, root_addr, root_contract, &owner, sign)?
        {
            return Ok(Some(hit));
        }
    }
    Ok(None)
}

fn check_static_call_targets(
    vmsta: &mut VMState,
    root_addr: &ContractAddress,
    root_contract: &ContractSto,
    height: u64,
) -> Rerr {
    let check_one = |func_tag: String, codes: &[u8], vmsta: &mut VMState| -> Rerr {
        let check_call = |call: CallSpec, vmsta: &mut VMState| -> Rerr {
            let sign = call.selector();
            let found = resolve_userfn_meta_by_lookup_for_check(
                vmsta,
                root_addr,
                root_contract,
                &func_tag,
                &call,
                &sign,
            )?;
            let Some((owner, meta)) = found else {
                return errf!(
                    "{}: call target function 0x{} not found",
                    func_tag,
                    hex::encode(sign)
                );
            };
            if call.requires_external_visibility() && !meta.is_external {
                return errf!(
                    "{}: target function 0x{} resolved in {} is not external",
                    func_tag,
                    hex::encode(sign),
                    owner.to_readable()
                );
            }
            Ok(())
        };
        scan_call_sites(codes, |inst, params| {
            check_call(
                decode_user_call_site(inst, params).map_err(|e| e.to_string())?,
                vmsta,
            )
        })
    };

    for f in root_contract.userfuncs.as_list() {
        let code_pkg = CodePkg::try_from(&f.code_stuff).map_err(|e| e.to_string())?;
        let ctype = code_pkg.code_type().map_err(|e| e.to_string())?;
        let codes = match ctype {
            CodeType::Bytecode => code_pkg.data,
            CodeType::IRNode => {
                runtime_irs_to_bytecodes(&code_pkg.data, height).map_err(|e| e.to_string())?
            }
        };
        let tag = format!("userfn 0x{}", hex::encode(f.sign.to_array()));
        check_one(tag, &codes, vmsta)?;
    }

    for f in root_contract.abstcalls.as_list() {
        let code_pkg = CodePkg::try_from(&f.code_stuff).map_err(|e| e.to_string())?;
        let ctype = code_pkg.code_type().map_err(|e| e.to_string())?;
        let codes = match ctype {
            CodeType::Bytecode => code_pkg.data,
            CodeType::IRNode => {
                runtime_irs_to_bytecodes(&code_pkg.data, height).map_err(|e| e.to_string())?
            }
        };
        let tag = format!("abstcall {}", f.sign[0]);
        check_one(tag, &codes, vmsta)?;
    }

    Ok(())
}

fn check_sub_contract_protocol_fee(
    ctx: &mut dyn Context,
    pfee: &Amount,
    charge_bytes: usize,
) -> Rerr {
    if pfee.is_negative() {
        return errf!("protocol fee cannot be negative");
    }
    if charge_bytes == 0 {
        return Ok(());
    }
    let min_fee = calc_contract_protocol_fee_min(ctx, charge_bytes)?;
    let maddr = ctx.env().tx.main;
    // check fee
    if pfee < &min_fee {
        return errf!(
            "protocol fee must be at least {} (bytes={}, periods={}) but got {}",
            &min_fee,
            charge_bytes,
            contract_store_perm_periods(ctx.env().block.height),
            pfee
        );
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
        return Ok(Amount::zero());
    }
    let periods = contract_store_perm_periods(ctx.env().block.height) as u128;
    let fee_purity = ctx.tx().gas_price_purity() as u128; // unit-238 per tx byte
    if periods == 0 || fee_purity == 0 {
        return errf!(
            "contract protocol fee calculate failed: periods={} fee_purity={}",
            periods,
            fee_purity
        );
    }
    let bytes = charge_bytes as u128;
    let Some(need) = fee_purity.checked_mul(bytes) else {
        return errf!(
            "contract protocol fee calculate failed: fee_purity * bytes overflow ({} * {})",
            fee_purity,
            bytes
        );
    };
    let Some(need) = need.checked_mul(periods) else {
        return errf!(
            "contract protocol fee calculate failed: required * periods overflow ({} * {})",
            need,
            periods
        );
    };
    Ok(Amount::coin_u128(need, UNIT_238))
}

/* ************************************* fn check_sub_contract_protocol_fee(ctx: &mut dyn Context, ctlsz: usize, ptcfee: &Amount) -> Rerr { // let _hei = ctx.env().block.height; let e = errf!("contract protocol fee calculate failed"); let mul = CONTRACT_STORE_FEE_MUL as u128; // 30 let feep = ctx.tx().fee_purity() as u128; // per-byte, no GSCU division let Some(rlfe) = feep.checked_mul(ctlsz as u128) else { return e }; let Some(rlfe) = rlfe.checked_mul(mul) else { return e }; let tx50fee = &Amount::coin_u128(rlfe, UNIT_238).compress(2, AmtCpr::Grow)?; if tx50fee <= ctx.tx().fee() { return e } println!("{}, {}, {}, {}", ctx.tx().size(), ctlsz, ctx.tx().fee(), tx50fee); let maddr = ctx.env().tx.main; // check fee if ptcfee < tx50fee { return errf!("protocol fee must need at least {} but just got {}", tx50fee, ptcfee) } operate::hac_sub(ctx, &maddr, ptcfee)?; Ok(()) } */

#[cfg(test)]
mod contract_test {
    use super::*;
    use crate::contract::{Abst, Contract};
    use basis::component::Env;
    use basis::interface::ActExec;
    use field::{Address, Amount, Uint4};
    use protocol::context::decode_gas_budget;
    use protocol::transaction::TransactionType3;
    use std::sync::Once;
    use sys::IntoTRet;
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
            let registry = protocol::setup::SetupBuilder::new()
                .block_hasher(|_, stuff| sys::calculate_hash(stuff))
                .action_register(protocol::action::register)
                .vm_assigner(|height| Box::new(crate::global_machine_manager().assign(height)))
                .build()
                .unwrap();
            protocol::setup::install_once(registry).unwrap();
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
        act.construct_call = Bool::new(true);
        act.contract = deploy_contract;
        let _ = act.execute(&mut ctx).into_tret()?;
        Ok(())
    }

    #[test]
    fn deploy_construct_inherits_parent_when_local_absent() {
        let main = test_main_addr();
        let parent_addr = test_contract(&main, 31);
        let child_nonce = 32;

        let parent = Contract::new()
            .syst(Abst::new(AbstCall::Construct).fitsh("return 1").unwrap())
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
            .syst(Abst::new(AbstCall::Construct).fitsh("return 1").unwrap())
            .into_sto();
        let child = Contract::new()
            .inh(parent_addr.to_addr())
            .syst(Abst::new(AbstCall::Construct).fitsh("return 0").unwrap())
            .into_sto();

        let res = run_deploy_with_preloaded(child_nonce, vec![(parent_addr, parent)], child);
        assert!(res.is_ok(), "local Construct must override inherited one");
    }

    #[test]
    fn deploy_rejects_parent_with_nested_inherit() {
        let main = test_main_addr();
        let grand_addr = test_contract(&main, 35);
        let parent_addr = test_contract(&main, 36);
        let child_nonce = 37;

        let grand = Contract::new().into_sto();
        let parent = Contract::new().inh(grand_addr.to_addr()).into_sto();
        let child = Contract::new().inh(parent_addr.to_addr()).into_sto();

        let err = run_deploy_with_preloaded(
            child_nonce,
            vec![(grand_addr, grand), (parent_addr, parent)],
            child,
        )
        .expect_err("deploy must reject parent with nested inherit");
        assert!(
            err.contains("cannot have parent inherit"),
            "unexpected deploy error: {err}"
        );
    }

    #[test]
    fn deploy_without_any_construct_returns_not_find() {
        let main = test_main_addr();
        let parent_addr = test_contract(&main, 38);
        let child_nonce = 39;

        let parent = Contract::new().into_sto();
        let child = Contract::new().inh(parent_addr.to_addr()).into_sto();

        let err = run_deploy_with_preloaded(child_nonce, vec![(parent_addr, parent)], child)
            .expect_err("deploy should fail when Construct is absent");
        assert!(
            err.contains("Construct") && err.contains("not found"),
            "unexpected deploy error: {err}"
        );
    }
}
