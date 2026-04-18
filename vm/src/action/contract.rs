/*
    Permanent storage pricing reference:
    - 0.0002 HAC / 200 bytes = 0.000001 HAC per byte
    - 1600 bytes * 10000 periods ~= 8 HAC total permanent protocol cost
    - 10000 periods ~= 9.51 years when one period = 100 blocks
*/
pub const CONTRACT_STORE_PERM_PERIODS: u64 = 10_000;
pub const CONTRACT_STORE_EDIT_PERIODS: u64 = 100;

/*
    Minimum protocol fee purity floor, in unit-238 per tx byte.
    10000:238 == 100:244 == 0.000001 HAC per byte.
*/
pub const CONTRACT_STORE_LOWEST_FEE_PURITY: i64 = 10000;

macro_rules! vmsto { ($ctx: expr) => {
    VMState::wrap($ctx.state())
}}


action_define! { ContractDeploy, 40,
    ActScope::TOP_ONLY_CAN_WITH_GUARD, 3, false, [],
    {
        protocol_cost: Amount
        nonce: Uint4
        construct_must: Bool
        construct_argv: BytesW2 // checked by SpaceCap::value_size at runtime
        _marks_:   Fixed4 // zero
        contract: ContractSto
    },
    (self, {
        format!("Deploy smart contract with nonce {}", *self.nonce)
    }),
    (self, ctx, _gas {
        if self._marks_.not_zero() { // compatibility for future
            return xerrf!("marks bytes invalid")
        }
        let hei = ctx.env().block.height;
        let (gst, cap) = peek_vm_runtime_limits(ctx, hei);
        let maddr = ctx.env().tx.main;
        // check contract
        let caddr = ContractAddress::calculate(&maddr, &self.nonce);
        if vmsto!(ctx).contract_exist(&caddr) {
            return xerrf!("contract {} already exists", (*caddr).to_readable())
        }
        // check
        self.contract.check(hei, &cap, &gst)?;
        if self.contract.metas.revision.uint() != 0 {
            return xerrf!("contract revision must be 0 on deploy")
        }
        precheck_contract_store(&caddr, &self.contract, &gst, ctx)?;
        if self.contract.size() == 0 {
            return xerrf!("contract content cannot be empty");
        }
        let charge_bytes = self.contract.size();
        // spend protocol fee
        check_sub_contract_protocol_cost(
            ctx,
            &self.protocol_cost,
            charge_bytes,
            contract_store_perm_periods(hei),
        )?;
        // save the contract first; tx-level rollback owns final unwind if Construct fails.
        vmsto!(ctx).contract_set_sync_edition(&caddr, &self.contract);
        let cargv = self.construct_argv.to_vec();
        if cargv.len() > cap.value_size {
            return xerrf!("construct argv size overflow")
        }
        if self.construct_must.check() {
            let _ = run_abst_entry(
                ctx,
                AbstCall::Construct,
                caddr.to_addr(),
                Value::Bytes(cargv),
            )?;
        }
        // ok finish
        Ok(vec![])
    })
}

action_define! { ContractUpdate, 41,
    ActScope::TOP_ONLY_CAN_WITH_GUARD, 3, false, [],
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
            return xerrf!("marks bytes invalid")
        }
        let hei = ctx.env().block.height;
        let (gst, cap) = peek_vm_runtime_limits(ctx, hei);
        // load old
        let caddr = ContractAddress::from_addr(self.address)?;
        let Some(contract) = vmsto!(ctx).contract(&caddr) else {
            return xerrf!("contract {} does not exist", (*caddr).to_readable())
        };
        // apply edit (in memory)
        let mut new_contract = contract.clone();
        let (_did_append, did_structural_change) = new_contract.apply_edit(&self.edit, hei, &cap, &gst)?;
        precheck_contract_store(&caddr, &new_contract, &gst, ctx)?;
        if new_contract.size() == 0 {
            return xerrf!("contract content cannot be empty");
        }
        let did_effective_lookup_change = effective_userfn_lookup_changed(
            &mut vmsto!(ctx),
            &caddr,
            &contract,
            &new_contract,
        )?;
        // Final dispatch is driven by whether any existing visible selector semantics changed.
        // Purely additive edits (e.g. inherit/library append, or new local funcs with no shadowing)
        // stay Append; structural replacements or selector-owner changes are Change.
        let is_change = did_structural_change || did_effective_lookup_change;
        // spend protocol fee: expansion fee (may be zero) + edited-bytes fee
        let old_size = contract.size();
        let new_size = new_contract.size();
        let delta_bytes = new_size.saturating_sub(old_size);
        let edit_bytes = self.edit.size();
        let expand_periods = contract_store_perm_periods(hei);
        let edit_periods = contract_store_edit_periods(hei);
        let expand_fee = calc_contract_protocol_cost_min_with_periods(ctx, delta_bytes, expand_periods)?;
        let edit_fee = calc_contract_protocol_cost_min_with_periods(ctx, edit_bytes, edit_periods)?;
        let total_fee = expand_fee.add_mode_u128(&edit_fee)?;
        let pcost = &self.protocol_cost;
        if pcost.is_negative() {
            return xerrf!("protocol fee cannot be negative");
        }
        if *pcost < total_fee {
            return xerrf!(
                "protocol fee must be at least {} (expand_bytes={}, expand_periods={}, edit_bytes={}, edit_periods={}) but got {}",
                &total_fee,
                delta_bytes,
                expand_periods,
                edit_bytes,
                edit_periods,
                &self.protocol_cost
            );
        }
        if !pcost.is_zero() {
            let maddr = ctx.env().tx.main;
            operate::hac_sub(ctx, &maddr, pcost)?;
        }
        let sys = maybe!(is_change, Change, Append); // Change or Append
        // Authorization is intentionally delegated to the current contract's Change/Append hook.
        // Run the selected hook on the current on-chain contract; failure means the update is not allowed.
        let _ = run_abst_entry(ctx, sys, caddr.to_addr(), Value::Nil)?;
        // save the new
        vmsto!(ctx).contract_set_sync_edition(&caddr, &new_contract);
        let caddr_real = caddr.to_addr();
        ctx.vm_invalidate_contract_cache(&caddr_real);
        Ok(vec![])
    })
}

/**************************************/

fn check_contract_self_reference(root_addr: &ContractAddress, root_contract: &ContractSto) -> Rerr {
    macro_rules! any_same { ($key: ident) => {
        root_contract.$key.as_list().iter().any(|a| a == root_addr)
    }}
    if any_same!(inherit) {
        return errf!("contract cannot inherit itself {}", root_addr.to_readable())
    }
    if any_same!(library) {
        return errf!("contract cannot link itself as library {}", root_addr.to_readable())
    }
    Ok(())
}

fn precheck_contract_links_and_calls(
    ctx: &mut dyn Context,
    root_addr: &ContractAddress,
    root_contract: &ContractSto,
    gst: &GasExtra,
) -> Rerr {
    let mut vmsta = VMState::wrap(ctx.state());
    check_link_contracts_exist(&mut vmsta, root_addr, root_contract)?;
    check_inherits_direct_parents_flat(&mut vmsta, root_addr, root_contract)?;
    check_static_call_targets(&mut vmsta, root_addr, root_contract, gst)?;
    Ok(())
}

fn precheck_contract_store(
    root_addr: &ContractAddress,
    root_contract: &ContractSto,
    gst: &GasExtra,
    ctx: &mut dyn Context,
) -> Rerr {
    check_contract_self_reference(root_addr, root_contract)?;
    precheck_contract_links_and_calls(ctx, root_addr, root_contract, gst)
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

fn collect_effective_userfn_owners(
    vmsta: &mut VMState,
    root_addr: &ContractAddress,
    root_contract: &ContractSto,
) -> Ret<std::collections::HashMap<FnSign, ContractAddress>> {
    let mut owners = std::collections::HashMap::new();
    for f in root_contract.userfuncs.as_list() {
        owners.entry(f.sign.to_array()).or_insert(root_addr.clone());
    }
    for parent in root_contract.inherit.as_list() {
        let sto = load_contract_for_check(vmsta, root_addr, root_contract, parent, "inherit")?;
        for f in sto.userfuncs.as_list() {
            owners.entry(f.sign.to_array()).or_insert(parent.clone());
        }
    }
    Ok(owners)
}

fn effective_userfn_lookup_changed(
    vmsta: &mut VMState,
    root_addr: &ContractAddress,
    old_contract: &ContractSto,
    new_contract: &ContractSto,
) -> Ret<bool> {
    let old_table = collect_effective_userfn_owners(vmsta, root_addr, old_contract)?;
    let new_table = collect_effective_userfn_owners(vmsta, root_addr, new_contract)?;
    for (sign, old_owner) in old_table {
        match new_table.get(&sign) {
            Some(new_owner) if new_owner == &old_owner => {}
            _ => return Ok(true),
        }
    }
    Ok(false)
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
    gst: &GasExtra,
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
                runtime_irs_to_bytecodes(&code_pkg.data, gst).map_err(|e| e.to_string())?
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
                runtime_irs_to_bytecodes(&code_pkg.data, gst).map_err(|e| e.to_string())?
            }
        };
        let tag = format!("abstcall {}", f.sign[0]);
        check_one(tag, &codes, vmsta)?;
    }

    Ok(())
}

fn check_sub_contract_protocol_cost(
    ctx: &mut dyn Context,
    pfee: &Amount,
    charge_bytes: usize,
    periods: u64,
) -> Rerr {
    if pfee.is_negative() {
        return errf!("protocol fee cannot be negative");
    }
    if charge_bytes == 0 {
        return Ok(());
    }
    let min_fee = calc_contract_protocol_cost_min_with_periods(ctx, charge_bytes, periods)?;
    let maddr = ctx.env().tx.main;
    if pfee < &min_fee {
        return errf!(
            "protocol fee must be at least {} (bytes={}, periods={}) but got {}",
            &min_fee,
            charge_bytes,
            periods,
            pfee
        );
    }
    operate::hac_sub(ctx, &maddr, pfee)?;
    Ok(())
}

#[inline(always)]
fn contract_store_perm_periods(_hei: u64) -> u64 {
    CONTRACT_STORE_PERM_PERIODS
}

#[inline(always)]
fn contract_store_edit_periods(_hei: u64) -> u64 {
    CONTRACT_STORE_EDIT_PERIODS
}

#[inline(always)]
fn effective_contract_fee_purity(ctx: &dyn Context) -> u64 {
    ctx.tx()
        .fee_purity()
        .max(CONTRACT_STORE_LOWEST_FEE_PURITY as u64)
}

fn calc_contract_protocol_cost_min_with_periods(
    ctx: &dyn Context,
    charge_bytes: usize,
    periods: u64,
) -> Ret<Amount> {
    if charge_bytes == 0 {
        return Ok(Amount::zero());
    }
    let fee_purity = effective_contract_fee_purity(ctx) as u128; // unit-238 per tx byte
    let periods = periods as u128;
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
    use crate::contract::{Abst, Contract, Func};
    use basis::component::Env;
    use basis::interface::ActExec;
    use field::{Address, Amount, Uint4};
    use protocol::context::decode_gas_budget;
    use protocol::transaction::TransactionType3;
    use std::cell::RefCell;
    use testkit::sim::context::make_ctx_with_state;
    use testkit::sim::state::FlatMemState as StateMem;
    use testkit::sim::tx::StubTxBuilder;

    fn test_main_addr() -> Address {
        Address::from_readable("1MzNY1oA3kfgYi75zquj3SRUPYztzXHzK9").unwrap()
    }

    fn test_contract(base: &Address, nonce: u32) -> ContractAddress {
        ContractAddress::calculate(base, &Uint4::from(nonce))
    }

    thread_local! {
        static VM_TEST_SETUP_SCOPE: RefCell<Option<protocol::setup::TestSetupScopeGuard>> = const { RefCell::new(None) };
    }

    fn init_vm_assigner_once() {
        let mut setup = protocol::setup::new_standard_protocol_setup(|_, stuff| sys::calculate_hash(stuff));
        mint::setup::register_protocol_extensions(&mut setup);
        crate::setup::register_protocol_extensions(&mut setup);
        let guard = protocol::setup::install_test_scope(setup);
        VM_TEST_SETUP_SCOPE.with(|cell| {
            *cell.borrow_mut() = Some(guard);
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
        ctx.gas_initialize(decode_gas_budget(17))?;

        let mut act = ContractDeploy::new();
        act.nonce = Uint4::from(nonce);
        act.protocol_cost = Amount::coin(10000, 244);
        act.construct_must = Bool::new(true);
        act.contract = deploy_contract;
        let _ = act.execute(&mut ctx)?;
        Ok(())
    }

    fn run_update_with_preloaded(
        target: ContractAddress,
        preload: Vec<(ContractAddress, ContractSto)>,
        edit: ContractEdit,
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
        env.chain.fast_sync = true;
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
        ctx.gas_initialize(decode_gas_budget(17))?;

        let mut act = ContractUpdate::new();
        act.address = target.to_addr();
        act.protocol_cost = Amount::coin(10000, 244);
        act.edit = edit;
        let _ = act.execute(&mut ctx)?;
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

    #[test]
    fn update_shadowing_parent_function_dispatches_change() {
        let main = test_main_addr();
        let parent_addr = test_contract(&main, 40);
        let child_addr = test_contract(&main, 41);

        let parent = Contract::new()
            .func(Func::new("f1").unwrap().fitsh("return 1").unwrap())
            .into_sto();
        let child = Contract::new()
            .inh(parent_addr.to_addr())
            .syst(Abst::new(AbstCall::Append).fitsh("return 1").unwrap())
            .syst(Abst::new(AbstCall::Change).fitsh("return 0").unwrap())
            .into_sto();
        let edit = Contract::new()
            .func(Func::new("f1").unwrap().fitsh("return 2").unwrap())
            .into_edit(1);

        run_update_with_preloaded(
            child_addr.clone(),
            vec![(parent_addr, parent), (child_addr, child)],
            edit,
        )
        .expect("shadowing parent selector should update successfully");
    }

    #[test]
    fn update_new_local_function_without_parent_conflict_stays_append() {
        let main = test_main_addr();
        let parent_addr = test_contract(&main, 42);
        let child_addr = test_contract(&main, 43);

        let parent = Contract::new()
            .func(Func::new("parent_only").unwrap().fitsh("return 1").unwrap())
            .into_sto();
        let child = Contract::new()
            .inh(parent_addr.to_addr())
            .syst(Abst::new(AbstCall::Append).fitsh("return 0").unwrap())
            .syst(Abst::new(AbstCall::Change).fitsh("return 1").unwrap())
            .into_sto();
        let edit = Contract::new()
            .func(Func::new("child_only").unwrap().fitsh("return 2").unwrap())
            .into_edit(1);

        run_update_with_preloaded(
            child_addr.clone(),
            vec![(parent_addr, parent), (child_addr, child)],
            edit,
        )
        .expect("new selector without parent conflict should update successfully");
    }
}
