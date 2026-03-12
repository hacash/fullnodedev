/// Per-tx VM call state. Gas ledger is stored in Context; VM only keeps
/// re-entry level guard and call-limit state.
#[derive(Clone)]
pub struct VmCallState {
    initialized: bool,  // whether init_once() has been called
    reentry_level: u32, // current ACTION re-entry level (0 = not in call)
    max_reentry: u32,   // hard cap from SpaceCap
}

struct VmReentryGuard<'a> {
    call_state: &'a mut VmCallState,
}

impl<'a> VmReentryGuard<'a> {
    fn enter(call_state: &'a mut VmCallState) -> Ret<Self> {
        call_state.enter()?;
        Ok(Self { call_state })
    }
}

impl Drop for VmReentryGuard<'_> {
    fn drop(&mut self) {
        self.call_state.leave();
    }
}

impl Default for VmCallState {
    fn default() -> Self {
        Self {
            initialized: false,
            reentry_level: 0,
            max_reentry: 4,
        }
    }
}

impl VmCallState {
    /// Initialize VM-side call limits only.
    fn init_once(&mut self, cap: &SpaceCap) -> Rerr {
        if self.initialized {
            return Ok(());
        }
        self.max_reentry = cap.reentry_level;
        self.initialized = true;
        Ok(())
    }

    /// Enter a call layer. Increments level, enforces re-entry limit.
    fn enter(&mut self) -> Rerr {
        let next_level = self
            .reentry_level
            .checked_add(1)
            .ok_or_else(|| "re-entry level overflow".to_owned())?;
        if next_level > self.max_reentry + 1 {
            // level 1 = outermost call, level 2 = first re-entry, etc.
            return errf!(
                "re-entry level {} exceeded limit {}",
                next_level - 1,
                self.max_reentry
            );
        }
        self.reentry_level = next_level;
        Ok(())
    }

    /// Leave a call layer. Decrements level.
    fn leave(&mut self) {
        self.reentry_level = self.reentry_level.saturating_sub(1);
    }
}

/*********************************/

#[allow(dead_code)]
pub struct MachineBox {
    call_state: VmCallState,
    machine: Option<Machine>,
}

impl Drop for MachineBox {
    fn drop(&mut self) {
        match self.machine.take() {
            Some(m) => global_machine_manager().reclaim(m.r),
            _ => (),
        }
    }
}

impl MachineBox {
    pub fn new(m: Machine) -> Self {
        Self {
            call_state: VmCallState::default(),
            machine: Some(m),
        }
    }
}

impl VM for MachineBox {
    fn snapshot_volatile(&self) -> Box<dyn Any> {
        let m = self.machine.as_ref().unwrap();
        // Snapshot excludes gas and gas-charged warmup/cache accounting so AST recover rolls back state/log/memory only while keeping gas/warmup monotonic.
        Box::new((m.r.global_map.clone(), m.r.memory_map.clone()))
    }

    fn restore_volatile(&mut self, snap: Box<dyn Any>) {
        let Ok(snap) = snap.downcast::<(GKVMap, CtcKVMap)>() else {
            return;
        };
        let (global_map, memory_map) = *snap;
        let m = self.machine.as_mut().unwrap();
        m.r.global_map = global_map;
        m.r.memory_map = memory_map;
        // Do not restore `r.contracts` because tx-local warmup/cache accounting tied to already-paid gas must stay monotonic across AST recover.
    }

    fn restore_but_keep_warmup(&mut self) {
        let m = self.machine.as_mut().unwrap();
        m.r.global_map.clear();
        m.r.memory_map.clear();
        // keep warmup/cache channel (`contracts`) and gas accounting monotonic.
    }

    fn invalidate_contract_cache(&mut self, addr: &Address) {
        let Ok(caddr) = ContractAddress::from_addr(*addr) else {
            return;
        };
        if let Some(m) = self.machine.as_mut() {
            m.r.contracts.remove(&caddr);
        }
        global_machine_manager()
            .contract_cache()
            .remove_addr(&caddr);
    }

    fn call(&mut self, call: VMCall<'_>) -> XRet<(i64, Vec<u8>)> {
        use EntryKind::*;
        let VMCall {
            ctx,
            entry,
            kind,
            payload,
            param,
        } = call;
        // (1) initialize gas budget on first call (idempotent)
        {
            let r = &self.machine.as_ref().unwrap().r;
            self.call_state.init_once(&r.space_cap)?;
        }
        // (2) enter call layer (depth check). Guard guarantees leave() on all exits.
        let _guard = VmReentryGuard::enter(&mut self.call_state)?;
        // min gas cost per call type
        let entry_kind = EntryKind::try_from_u8(entry).map_err(XError::from)?;
        let min_cost = {
            let gsext = &self.machine.as_ref().unwrap().r.gas_extra;
            match entry_kind {
                Main => gsext.main_call_min,
                P2sh => gsext.p2sh_call_min,
                Abst => gsext.abst_call_min,
            }
        };
        // (3) execute VM call with shared gas counter
        let gas_before = ctx.gas_remaining();
        // Fail-fast: if remaining gas can't cover the per-call minimum, this call cannot start.
        if gas_before < min_cost {
            let gas = ctx
                .vm_gas_mut()
                .into_xret()?
                .gas_remaining_mut()
                .into_xret()?;
            *gas -= min_cost; // keep the same "min cost consumes from shared counter" semantics
            return xerrf!(
                "gas budget too low: remaining={} < min_call_cost={} (mode={:?})",
                gas_before,
                min_cost,
                entry_kind
            );
        }
        let machine = self.machine.as_mut().unwrap();
        let ctxptr = ctx as *mut dyn Context;
        let gasptr = unsafe {
            let gasctx = (*ctxptr).vm_gas_mut().into_xret()?;
            gasctx.gas_remaining_mut().into_xret()? as *mut i64
        };
        let exenv = unsafe {
            &mut ExecEnv {
                ctx: &mut *ctxptr,
                gas: &mut *gasptr,
            }
        };
        let result = match entry_kind {
            Main => {
                let codeconf = CodeConf::parse(kind)?;
                machine.main_call(exenv, codeconf.code_type(), payload)
            }
            P2sh => {
                let codeconf = CodeConf::parse(kind)?;
                let payload = ByteView::from_arc(payload);
                let payload_ref = payload.as_slice();
                let (state_addr, mv1) = Address::create(payload_ref).map_err(XError::fault)?;
                let (calibs, mv2) =
                    ContractAddressW1::create(&payload_ref[mv1..]).map_err(XError::fault)?;
                let mv = mv1 + mv2;
                let realcodes = payload
                    .slice(mv, payload.len())
                    .map_err(XError::fault)?;
                let Ok(param) = param.downcast::<Value>() else {
                    return xerrf!("p2sh argv type not match");
                };
                machine.p2sh_call(
                    exenv,
                    codeconf.code_type(),
                    state_addr,
                    calibs.into_list(),
                    realcodes,
                    *param,
                )
            }
            Abst => {
                let kid = AbstCall::try_from_u8(kind).map_err(XError::from)?;
                let cadr = ContractAddress::parse(payload.as_ref()).map_err(XError::fault)?;
                let Ok(param) = param.downcast::<Value>() else {
                    return xerrf!("abst argv type not match");
                };
                machine.abst_call(exenv, kid, cadr, *param)
            }
        };
        // (4) compute gas cost, enforce minimum, leave call layer
        let gas_after = ctx.gas_remaining();
        let actual = gas_before - gas_after;
        let mut cost = actual;
        // enforce per-call minimum gas by consuming shortfall from shared counter
        if cost < min_cost {
            let shortfall = min_cost - cost;
            let gas = ctx
                .vm_gas_mut()
                .into_xret()?
                .gas_remaining_mut()
                .into_xret()?;
            *gas -= shortfall;
            if *gas < 0 {
                return xerrf!(
                    "gas has run out after min cost enforcement: remaining={} (before={} min_call_cost={} actual_cost={})",
                    *gas,
                    gas_before,
                    min_cost,
                    actual
                );
            }
            cost = min_cost;
        }
        // propagate VM execution error (depth is auto-restored by guard drop)
        let resv = result.map(|a| a.raw())?;
        if cost <= 0 {
            return xerrf!("gas cost error: {}", cost);
        }
        Ok((cost, resv))
    }
}

/*********************************/

#[allow(dead_code)]
pub struct Machine {
    r: Resoure,
    frames: Vec<CallFrame>,
}

impl Machine {
    pub fn is_in_calling(&self) -> bool {
        !self.frames.is_empty()
    }

    pub fn create(r: Resoure) -> Self {
        Self { r, frames: vec![] }
    }

    pub fn main_call_raw(
        &mut self,
        env: &mut ExecEnv,
        ctype: CodeType,
        codes: Arc<[u8]>,
    ) -> Ret<Value> {
        // Caller must pre-validate code bytes. Production entry actions run convert_and_check before setup_vm_run.
        let fnobj = FnObj::plain(ctype, codes, 0, None);
        let ctx_adr = env.ctx.tx().main();
        let lib_adr = env.ctx.env().tx.addrs.clone();
        Ok(self.do_call(
            env,
            ExecCtx::main(),
            &fnobj,
            FrameBindings::root(ctx_adr, lib_adr.into()),
            None,
        )?)
    }

    pub fn main_call(
        &mut self,
        env: &mut ExecEnv,
        ctype: CodeType,
        codes: Arc<[u8]>,
    ) -> XRet<Value> {
        let rv = self.main_call_raw(env, ctype, codes).into_xret()?;
        check_vm_return_value(&rv, "main call")?;
        Ok(rv)
    }

    pub fn abst_call(
        &mut self,
        env: &mut ExecEnv,
        cty: AbstCall,
        contract_addr: ContractAddress,
        param: Value,
    ) -> XRet<Value> {
        let exec = ExecCtx::abst();
        exec.ensure_call_depth(&self.r.space_cap).map_err(XError::from)?;
        param.canbe_func_argv().map_err(XError::from)?;
        let adr = contract_addr.to_readable();
        let Some(hit) = self
            .r
            .resolve_abstfn(env.ctx, env.gas, &contract_addr, cty)
            .map_err(XError::from)?
        else {
            return Err(XError::fault(format!("abst call {:?} not find in {}", cty, adr)));
        };
        // Keep state anchored to the concrete contract address, even when abstract entry body is inherited from a parent owner. This preserves this/self split semantics.
        let rv = self.do_call(
            env,
            exec,
            hit.fnobj.as_ref(),
            FrameBindings::contract(contract_addr, hit.owner, hit.lib_table),
            Some(param),
        ).map_err(XError::from)?;
        check_vm_return_value(&rv, &format!("call {}.{:?}", adr, cty))?;
        Ok(rv)
    }

    fn p2sh_call(
        &mut self,
        env: &mut ExecEnv,
        ctype: CodeType,
        p2sh_addr: Address,
        libs: Vec<ContractAddress>,
        codes: ByteView,
        param: Value,
    ) -> XRet<Value> {
        // Caller must pre-validate lock script bytes. Production P2SH flow verifies inputs before VM call.
        let fnobj = FnObj::plain(ctype, codes, 0, None);
        let ctx_adr = p2sh_addr;
        let rv = self.do_call(
            env,
            ExecCtx::p2sh(),
            &fnobj,
            FrameBindings::root(
                ctx_adr,
                libs.into_iter()
                    .map(|addr| addr.into_addr())
                    .collect::<Vec<_>>()
                    .into(),
            ),
            Some(param),
        ).map_err(XError::from)?;
        check_vm_return_value(&rv, "p2sh call")?;
        Ok(rv)
    }

    fn do_call(
        &mut self,
        env: &mut ExecEnv,
        exec: ExecCtx,
        code: &FnObj,
        bindings: FrameBindings,
        param: Option<Value>,
    ) -> VmrtRes<Value> {
        self.frames.push(CallFrame::new());
        let res = self.frames.last_mut().unwrap().start_call(
            &mut self.r,
            env,
            exec,
            code,
            bindings,
            param,
        );
        self.frames.pop().unwrap().reclaim(&mut self.r);
        res
    }
}

#[cfg(test)]
mod machine_test {

    use super::*;
    use crate::contract::{Abst, Contract, Func};
    use crate::lang::lang_to_bytecode;
    use crate::rt::CodeType;

    use crate::value::ValueTy as VT;
    use basis::component::Env;
    use basis::interface::Context;
    use field::{Address, Amount, Uint4};
    use testkit::sim::context::make_ctx_with_state;
    use testkit::sim::state::FlatMemState as StateMem;
    use testkit::sim::tx::StubTxBuilder;

    fn test_base_addr() -> Address {
        Address::from_readable("1MzNY1oA3kfgYi75zquj3SRUPYztzXHzK9").unwrap()
    }

    fn test_contract(base: &Address, nonce: u32) -> crate::ContractAddress {
        crate::ContractAddress::calculate(base, &Uint4::from(nonce))
    }

    fn run_main_script_with(
        base_addr: Address,
        tx_libs: Vec<crate::ContractAddress>,
        mut ext_state: StateMem,
        main_script: &str,
        raw: bool,
    ) -> Ret<Value> {
        let main_codes = lang_to_bytecode(main_script).unwrap();
        let mut env = Env::default();
        env.block.height = 1;
        env.tx.main = base_addr;
        env.tx.addrs = tx_libs.iter().map(|a| a.clone().into_addr()).collect();

        let tx = StubTxBuilder::new()
            .ty(0)
            .main(base_addr)
            .addrs(env.tx.addrs.clone())
            .fee(Amount::zero())
            .gas_max(1)
            .tx_size(128)
            .fee_purity(1)
            .build();
        let mut ctx = make_ctx_with_state(env, Box::new(std::mem::take(&mut ext_state)), &tx);

        let mut gas: i64 = 1_000_000;
        let mut exec = crate::frame::ExecEnv {
            ctx: &mut ctx as &mut dyn Context,
            gas: &mut gas,
        };
        let mut machine = Machine::create(Resoure::create(1));
        if raw {
            machine.main_call_raw(&mut exec, CodeType::Bytecode, main_codes.into())
        } else {
            machine.main_call(&mut exec, CodeType::Bytecode, main_codes.into()).into_tret()
        }
    }

    fn run_main_script(
        base_addr: Address,
        tx_libs: Vec<crate::ContractAddress>,
        ext_state: StateMem,
        main_script: &str,
    ) -> Ret<Value> {
        run_main_script_with(base_addr, tx_libs, ext_state, main_script, false)
    }

    fn run_main_script_raw(
        base_addr: Address,
        tx_libs: Vec<crate::ContractAddress>,
        ext_state: StateMem,
        main_script: &str,
    ) -> Ret<Value> {
        run_main_script_with(base_addr, tx_libs, ext_state, main_script, true)
    }

    fn run_p2sh_script(
        p2sh_addr: Address,
        tx_libs: Vec<crate::ContractAddress>,
        mut ext_state: StateMem,
        p2sh_script: &str,
    ) -> Ret<Value> {
        let p2sh_codes = lang_to_bytecode(p2sh_script).unwrap();
        let main_addr = test_base_addr();
        let mut env = Env::default();
        env.block.height = 1;
        env.tx.main = main_addr;
        env.tx.addrs = tx_libs.iter().map(|a| a.clone().into_addr()).collect();

        let tx = StubTxBuilder::new()
            .ty(0)
            .main(main_addr)
            .addrs(env.tx.addrs.clone())
            .fee(Amount::zero())
            .gas_max(1)
            .tx_size(128)
            .fee_purity(1)
            .build();
        let mut ctx = make_ctx_with_state(env, Box::new(std::mem::take(&mut ext_state)), &tx);

        let mut gas: i64 = 1_000_000;
        let mut exec = crate::frame::ExecEnv {
            ctx: &mut ctx as &mut dyn Context,
            gas: &mut gas,
        };
        let mut machine = Machine::create(Resoure::create(1));
        machine.p2sh_call(
            &mut exec,
            CodeType::Bytecode,
            p2sh_addr,
            tx_libs,
            p2sh_codes.into(),
            Value::Nil,
        ).into_tret()
    }

    fn assert_err_contains(res: Ret<Value>, needle: &str) {
        let err = res.expect_err("expected error");
        assert!(
            err.contains(needle),
            "expected error to contain '{needle}', got '{err}'"
        );
    }

    #[test]
    fn main_call_raw_accepts_object_return() {
        let base_addr = test_base_addr();
        let rv = run_main_script_raw(
            base_addr,
            vec![],
            StateMem::default(),
            r##"
                return map { "kind": "hnft", "mint": 1 }
            "##,
        )
        .unwrap();
        assert_eq!(rv.to_json(), r#"{"kind":"hnft","mint":1}"#);
    }

    #[test]
    fn main_call_still_rejects_object_return() {
        let base_addr = test_base_addr();
        let res = run_main_script(
            base_addr,
            vec![],
            StateMem::default(),
            r##"
                return map { "kind": "hnft" }
            "##,
        );
        assert_err_contains(res, "main call return error");
    }

    #[test]
    fn main_call_still_rejects_args_return() {
        let base_addr = test_base_addr();
        let res = run_main_script(
            base_addr,
            vec![],
            StateMem::default(),
            r##"
                return args(7, 8)
            "##,
        );
        assert_err_contains(res, "main call return error");
    }

    #[test]
    fn sandbox_call_displays_object_return() {
        let base_addr = test_base_addr();
        let contract = test_contract(&base_addr, 31);
        let contract_sto = Contract::new()
            .func(
                Func::new("probe")
                    .unwrap()
                    .external()
                    .fitsh(r##"return map { "kind": "hnft", "mint": 1 }"##)
                    .unwrap(),
            )
            .into_sto();
        let mut ext_state = StateMem::default();
        {
            let mut vmsta = crate::VMState::wrap(&mut ext_state);
            vmsta.contract_set_sync_edition(&contract, &contract_sto);
        }

        let mut env = Env::default();
        env.block.height = 1;
        env.tx.main = base_addr;
        let tx = StubTxBuilder::new()
            .ty(0)
            .main(base_addr)
            .fee(Amount::zero())
            .gas_max(1)
            .tx_size(128)
            .fee_purity(1)
            .build();
        let mut ctx = make_ctx_with_state(env, Box::new(ext_state), &tx);

        let (_gas, rv) = sandbox_call(&mut ctx, contract, "probe".to_owned(), "").unwrap();
        assert_eq!(rv, r#"{"kind":"hnft","mint":1}"#);
    }

    #[test]
    fn calltargets_resolve_under_ext_view_and_inherits() {
        // Arrange addresses.
        let base_addr = Address::from_readable("1MzNY1oA3kfgYi75zquj3SRUPYztzXHzK9").unwrap();
        let contract_child = crate::ContractAddress::calculate(&base_addr, &Uint4::from(1));
        let contract_parent = crate::ContractAddress::calculate(&base_addr, &Uint4::from(2));
        let contract_base = crate::ContractAddress::calculate(&base_addr, &Uint4::from(3));

        // Build an inheritance chain: Child -> Parent -> Base. The key trick is: `super.f()` moves code_owner to Parent, while state_addr stays Child. Then inside Parent.f(), `this.g()` must resolve in state_addr (Child), `self.g()` in code_owner (Parent), and `super.g()` in Parent's direct base (Base).

        let base = Contract::new().func(Func::new("g").unwrap().fitsh("return 3").unwrap());

        let parent = Contract::new()
            .inh(contract_base.to_addr())
            .func(Func::new("g").unwrap().fitsh("return 2").unwrap())
            .func(
                Func::new("f")
                    .unwrap()
                    .fitsh(
                        r##"
                        return this.g() * 10000 + self.g() * 100 + super.g()
                        "##,
                    )
                    .unwrap(),
            );

        let child = Contract::new()
            .inh(contract_parent.to_addr())
            .func(Func::new("g").unwrap().fitsh("return 1").unwrap())
            .func(
                Func::new("run")
                    .unwrap()
                    .external()
                    .fitsh(
                        r##"
                        let v = super.f()
                        assert v == 10203
                        return 0
                        "##,
                    )
                    .unwrap(),
            );

        // Put contracts into state, then move it into Context.
        let mut ext_state = StateMem::default();
        {
            let mut vmsta = crate::VMState::wrap(&mut ext_state);
            vmsta.contract_set_sync_edition(&contract_base, &base.into_sto());
            vmsta.contract_set_sync_edition(&contract_parent, &parent.into_sto());
            vmsta.contract_set_sync_edition(&contract_child, &child.into_sto());
        }

        // Main script calls contract_main.run() using tx-provided libs (index 0).
        let main_script = r##"
            lib C = 0
            return C.run()
        "##;
        let main_codes = lang_to_bytecode(main_script).unwrap();

        let mut env = Env::default();
        env.block.height = 1;
        env.tx.main = base_addr;
        env.tx.addrs = vec![contract_child.into_addr()];

        let tx = StubTxBuilder::new()
            .ty(0)
            .main(base_addr)
            .addrs(env.tx.addrs.clone())
            .fee(Amount::zero())
            .gas_max(1)
            .tx_size(128)
            .fee_purity(1)
            .build();
        let mut ctx = make_ctx_with_state(env, Box::new(ext_state), &tx);

        let mut gas: i64 = 1_000_000;
        let mut exec = crate::frame::ExecEnv {
            ctx: &mut ctx as &mut dyn Context,
            gas: &mut gas,
        };

        let mut machine = Machine::create(Resoure::create(1));
        let rv = machine
            .main_call(&mut exec, CodeType::Bytecode, main_codes.into())
            .unwrap();

        assert!(
            !rv.canbe_bool().unwrap(),
            "main call should return success (nil/0)"
        );
    }

    #[test]
    fn internal_contract_call_accepts_args_return_value() {
        let base_addr = test_base_addr();
        let contract = test_contract(&base_addr, 88);
        let contract_sto = Contract::new()
            .func(
                Func::new("build")
                    .unwrap()
                    .types(Some(VT::Args), vec![])
                    .fitsh(r##"return args(7, map { "kind": "hnft" })"##)
                    .unwrap(),
            )
            .func(
                Func::new("consume")
                    .unwrap()
                    .types(Some(VT::U8), vec![VT::U8, VT::Compo])
                    .fitsh(
                        r##"
                        param { num doc }
                        assert doc is map
                        return num
                    "##,
                    )
                    .unwrap(),
            )
            .func(
                Func::new("relay")
                    .unwrap()
                    .external()
                    .types(Some(VT::U8), vec![])
                    .fitsh(r##"return this.consume(this.build())"##)
                    .unwrap(),
            )
            .into_sto();
        let mut ext_state = StateMem::default();
        {
            let mut vmsta = crate::VMState::wrap(&mut ext_state);
            vmsta.contract_set_sync_edition(&contract, &contract_sto);
        }
        let rv = run_main_script_raw(
            base_addr,
            vec![contract],
            ext_state,
            r##"
                lib C = 0
                return C.relay()
            "##,
        )
        .unwrap();
        assert_eq!(rv, Value::U8(7));
    }

    #[test]
    fn call_external_view_pure_use_inherits_but_codecall_keeps_local_lookup() {
        let base_addr = Address::from_readable("1MzNY1oA3kfgYi75zquj3SRUPYztzXHzK9").unwrap();
        let contract_child = crate::ContractAddress::calculate(&base_addr, &Uint4::from(11));
        let contract_parent = crate::ContractAddress::calculate(&base_addr, &Uint4::from(12));

        let parent_sto = Contract::new()
            .func(Func::new("id").unwrap().fitsh("return 2").unwrap())
            .func(
                Func::new("probe")
                    .unwrap()
                    .external()
                    .fitsh("return 201")
                    .unwrap(),
            )
            .into_sto();
        let child_sto = Contract::new()
            .inh(contract_parent.to_addr())
            .func(Func::new("id").unwrap().fitsh("return 1").unwrap())
            .func(
                Func::new("noop")
                    .unwrap()
                    .external()
                    .fitsh("return 0")
                    .unwrap(),
            )
            .into_sto();

        let run_main = |main_script: &str| -> Ret<Value> {
            let main_codes = lang_to_bytecode(main_script).unwrap();
            let mut ext_state = StateMem::default();
            {
                let mut vmsta = crate::VMState::wrap(&mut ext_state);
                vmsta.contract_set_sync_edition(&contract_parent, &parent_sto.clone());
                vmsta.contract_set_sync_edition(&contract_child, &child_sto.clone());
            }

            let mut env = Env::default();
            env.block.height = 1;
            env.tx.main = base_addr;
            env.tx.addrs = vec![contract_child.clone().into_addr()];

            let tx = StubTxBuilder::new()
                .ty(0)
                .main(base_addr)
                .addrs(env.tx.addrs.clone())
                .fee(Amount::zero())
                .gas_max(1)
                .tx_size(128)
                .fee_purity(1)
                .build();
            let mut ctx = make_ctx_with_state(env, Box::new(ext_state), &tx);

            let mut gas: i64 = 1_000_000;
            let mut exec = crate::frame::ExecEnv {
                ctx: &mut ctx as &mut dyn Context,
                gas: &mut gas,
            };
            let mut machine = Machine::create(Resoure::create(1));
            machine.main_call(&mut exec, CodeType::Bytecode, main_codes.into()).into_tret()
        };

        // CALLEXT (External): should resolve inherited `probe` on parent.
        let external_script = r##"
            lib C = 0
            let v = C.probe()
            assert v == 201
            return 0
        "##;
        assert!(
            run_main(external_script).is_ok(),
            "CALLEXT should resolve through inherit chain"
        );

        // CALLEXTVIEW and ext-pure generic calls should also resolve the inherit chain; CODECALL stays local-only.
        let view_script = r##"
            lib C = 0
            assert C:probe() == 201
            return 0
        "##;
        assert!(
            run_main(view_script).is_ok(),
            "CALLEXTVIEW should resolve through inherit chain"
        );

        let pure_script = r##"
            lib C = 0
            assert call pure C.probe() == 201
            return 0
        "##;
        assert!(
            run_main(pure_script).is_ok(),
            "generic Ext(lib)+Pure should resolve through inherit chain"
        );

        let codecall_script = r##"
            lib C = 0
            codecall C.probe
        "##;
        assert!(
            run_main(codecall_script).is_err(),
            "CODECALL should not resolve inherit chain"
        );
    }

    #[test]
    fn call_external_requires_external_visibility() {
        let base_addr = test_base_addr();
        let contract_target = test_contract(&base_addr, 21);

        let target_sto = Contract::new()
            .func(Func::new("hidden").unwrap().fitsh("return 1").unwrap())
            .into_sto();
        let mut ext_state = StateMem::default();
        {
            let mut vmsta = crate::VMState::wrap(&mut ext_state);
            vmsta.contract_set_sync_edition(&contract_target, &target_sto);
        }

        let script = r##"
            lib C = 0
            return C.hidden()
        "##;
        let res = run_main_script(base_addr, vec![contract_target], ext_state, script);
        assert_err_contains(res, "CallNotExternal");
    }

    #[test]
    fn call_libidx_overflow_is_reported() {
        let base_addr = test_base_addr();
        let script = r##"
            lib C = 0
            return C.anything()
        "##;
        let res = run_main_script(base_addr, vec![], StateMem::default(), script);
        assert_err_contains(res, "CallLibIdxOverflow");
    }

    #[test]
    fn callthis_callself_callsuper_are_forbidden_in_main_mode() {
        let base_addr = test_base_addr();
        let scripts = [
            "return this.nope()",
            "return self.nope()",
            "return super.nope()",
        ];
        for sc in scripts {
            let res = run_main_script(base_addr, vec![], StateMem::default(), sc);
            assert_err_contains(res, "CallOtherInMain");
        }
    }

    #[test]
    fn abst_this_and_self_follow_state_addr_and_code_owner() {
        let base_addr = test_base_addr();
        let contract_child = test_contract(&base_addr, 28);
        let contract_parent = test_contract(&base_addr, 29);

        let parent_sto = Contract::new()
            .syst(
                Abst::new(AbstCall::Construct)
                    .fitsh(
                        r##"
                        let v = this.id() * 100 + self.id()
                        assert v == 102
                        return 0
                        "##,
                    )
                    .unwrap(),
            )
            .func(Func::new("id").unwrap().fitsh("return 2").unwrap())
            .into_sto();
        let child_sto = Contract::new()
            .inh(contract_parent.to_addr())
            .func(Func::new("id").unwrap().fitsh("return 1").unwrap())
            .into_sto();

        let mut ext_state = StateMem::default();
        {
            let mut vmsta = crate::VMState::wrap(&mut ext_state);
            vmsta.contract_set_sync_edition(&contract_parent, &parent_sto);
            vmsta.contract_set_sync_edition(&contract_child, &child_sto);
        }

        let mut env = Env::default();
        env.block.height = 1;
        env.tx.main = base_addr;

        let tx = StubTxBuilder::new()
            .ty(0)
            .main(base_addr)
            .fee(Amount::zero())
            .gas_max(1)
            .tx_size(128)
            .fee_purity(1)
            .build();
        let mut ctx = make_ctx_with_state(env, Box::new(ext_state), &tx);

        let mut gas: i64 = 1_000_000;
        let mut exec = crate::frame::ExecEnv {
            ctx: &mut ctx as &mut dyn Context,
            gas: &mut gas,
        };
        let mut machine = Machine::create(Resoure::create(1));
        let rv = machine
            .abst_call(
                &mut exec,
                AbstCall::Construct,
                contract_child,
                Value::Bytes(vec![]),
            )
            .unwrap();
        assert!(
            !rv.canbe_bool().unwrap(),
            "abst call should finish without assertion failure"
        );
    }

    #[test]
    fn abst_call_first_cold_load_costs_more_than_second_warm_call() {
        let base_addr = test_base_addr();
        let contract_child = test_contract(&base_addr, 31);
        let contract_parent = test_contract(&base_addr, 32);

        let parent_sto = Contract::new()
            .syst(Abst::new(AbstCall::Construct).fitsh("return 0").unwrap())
            .into_sto();
        let child_sto = Contract::new().inh(contract_parent.to_addr()).into_sto();

        let mut ext_state = StateMem::default();
        {
            let mut vmsta = crate::VMState::wrap(&mut ext_state);
            vmsta.contract_set_sync_edition(&contract_parent, &parent_sto);
            vmsta.contract_set_sync_edition(&contract_child, &child_sto);
        }

        let mut env = Env::default();
        env.block.height = 1;
        env.tx.main = base_addr;

        let tx = StubTxBuilder::new()
            .ty(0)
            .main(base_addr)
            .fee(Amount::zero())
            .gas_max(1)
            .tx_size(128)
            .fee_purity(1)
            .build();
        let mut ctx = make_ctx_with_state(env, Box::new(ext_state), &tx);
        let mut machine = Machine::create(Resoure::create(1));
        let gas_budget = 1_000_000i64;

        let mut gas_1 = gas_budget;
        let mut exec_1 = crate::frame::ExecEnv {
            ctx: &mut ctx as &mut dyn Context,
            gas: &mut gas_1,
        };
        machine
            .abst_call(
                &mut exec_1,
                AbstCall::Construct,
                contract_child.clone(),
                Value::Bytes(vec![]),
            )
            .unwrap();
        let used_1 = gas_budget - gas_1;

        let mut gas_2 = gas_budget;
        let mut exec_2 = crate::frame::ExecEnv {
            ctx: &mut ctx as &mut dyn Context,
            gas: &mut gas_2,
        };
        machine
            .abst_call(
                &mut exec_2,
                AbstCall::Construct,
                contract_child,
                Value::Bytes(vec![]),
            )
            .unwrap();
        let used_2 = gas_budget - gas_2;

        assert!(
            used_1 > used_2,
            "first cold abst_call should consume more gas than second warm call"
        );
    }

    #[test]
    fn abst_external_call_fails_before_loading_lib_target() {
        let base_addr = test_base_addr();
        let contract_entry = test_contract(&base_addr, 33);
        let contract_target = test_contract(&base_addr, 34);

        let target_sto = Contract::new()
            .func(
                Func::new("probe")
                    .unwrap()
                    .external()
                    .fitsh("return 7")
                    .unwrap(),
            )
            .into_sto();
        let entry_sto = Contract::new()
            .lib(contract_target.to_addr())
            .syst(
                Abst::new(AbstCall::Construct)
                    .fitsh(
                        r##"
                        lib T = 0
                        return T.probe()
                        "##,
                    )
                    .unwrap(),
            )
            .into_sto();

        let mut ext_state = StateMem::default();
        {
            let mut vmsta = crate::VMState::wrap(&mut ext_state);
            vmsta.contract_set_sync_edition(&contract_entry, &entry_sto);
            vmsta.contract_set_sync_edition(&contract_target, &target_sto);
        }

        let mut env = Env::default();
        env.block.height = 1;
        env.tx.main = base_addr;

        let tx = StubTxBuilder::new()
            .ty(0)
            .main(base_addr)
            .fee(Amount::zero())
            .gas_max(1)
            .tx_size(128)
            .fee_purity(1)
            .build();
        let mut ctx = make_ctx_with_state(env, Box::new(ext_state), &tx);
        let mut gas: i64 = 1_000_000;
        let mut exec = crate::frame::ExecEnv {
            ctx: &mut ctx as &mut dyn Context,
            gas: &mut gas,
        };
        let mut machine = Machine::create(Resoure::create(1));

        let err = machine
            .abst_call(
                &mut exec,
                AbstCall::Construct,
                contract_entry.clone(),
                Value::Bytes(vec![]),
            )
            .expect_err("abst external call must be rejected before target load");
        assert!(err.contains("CallInAbst"), "unexpected error: {err}");
        assert!(machine.r.contracts.contains_key(&contract_entry));
        assert!(!machine.r.contracts.contains_key(&contract_target));
    }

    #[test]
    fn callextview_and_ext_pure_enforce_mode_call_whitelist() {
        let base_addr = test_base_addr();
        let contract_target = test_contract(&base_addr, 22);
        let target_sto = Contract::new()
            .func(
                Func::new("bad_view")
                    .unwrap()
                    .external()
                    .fitsh("return this.nope()")
                    .unwrap(),
            )
            .func(
                Func::new("bad_pure")
                    .unwrap()
                    .external()
                    .fitsh("return this.nope()")
                    .unwrap(),
            )
            .into_sto();
        let mut ext_state = StateMem::default();
        {
            let mut vmsta = crate::VMState::wrap(&mut ext_state);
            vmsta.contract_set_sync_edition(&contract_target, &target_sto);
        }

        let view_script = r##"
            lib C = 0
            return C:bad_view()
        "##;
        let pure_script = r##"
            lib C = 0
            return C::bad_pure()
        "##;

        let view_res = run_main_script(
            base_addr.clone(),
            vec![contract_target.clone()],
            ext_state.clone(),
            view_script,
        );
        assert_err_contains(view_res, "CallLocInView");

        let pure_res = run_main_script(base_addr, vec![contract_target], ext_state, pure_script);
        assert_err_contains(pure_res, "CallInPure");
    }

    #[test]
    fn codecall_cannot_reenable_action_from_nested_frame() {
        let base_addr = test_base_addr();
        let contract_target = test_contract(&base_addr, 221);
        let target_sto = Contract::new()
            .func(
                Func::new("bad_act")
                    .unwrap()
                    .fitsh(
                        r##"
                        transfer_hac_to(1MzNY1oA3kfgYi75zquj3SRUPYztzXHzK9, 1)
                        return 0
                        "##,
                    )
                    .unwrap(),
            )
            .into_sto();
        let mut ext_state = StateMem::default();
        {
            let mut vmsta = crate::VMState::wrap(&mut ext_state);
            vmsta.contract_set_sync_edition(&contract_target, &target_sto);
        }

        let script = r##"
            lib C = 0
            codecall C.bad_act
        "##;
        let res = run_main_script(base_addr, vec![contract_target], ext_state, script);
        assert_err_contains(res, "ActDisabled");
    }

    #[test]
    fn p2sh_codecall_cannot_reenable_nested_edit_call() {
        let base_addr = test_base_addr();
        let contract_entry = test_contract(&base_addr, 231);
        let contract_target = test_contract(&base_addr, 232);

        let target_sto = Contract::new()
            .func(
                Func::new("id")
                    .unwrap()
                    .external()
                    .fitsh("return 0")
                    .unwrap(),
            )
            .into_sto();
        let entry_sto = Contract::new()
            .lib(contract_target.to_addr())
            .func(
                Func::new("jump_ext")
                    .unwrap()
                    .fitsh(
                        r##"
                        lib Dep = 0
                        assert Dep.id() == 0
                        return 0
                        "##,
                    )
                    .unwrap(),
            )
            .into_sto();

        let mut ext_state = StateMem::default();
        {
            let mut vmsta = crate::VMState::wrap(&mut ext_state);
            vmsta.contract_set_sync_edition(&contract_target, &target_sto);
            vmsta.contract_set_sync_edition(&contract_entry, &entry_sto);
        }

        let script = r##"
            lib C = 0
            codecall C.jump_ext
        "##;
        let res = run_p2sh_script(
            Address::create_scriptmh([23u8; 20]),
            vec![contract_entry],
            ext_state,
            script,
        );
        assert_err_contains(res, "CallOtherInP2sh");
    }

    #[test]
    fn p2sh_codecall_still_allows_nested_view_and_pure_calls() {
        let base_addr = test_base_addr();
        let contract_entry = test_contract(&base_addr, 233);
        let contract_target = test_contract(&base_addr, 234);

        let target_sto = Contract::new()
            .func(Func::new("view_id").unwrap().fitsh("return 7").unwrap())
            .func(Func::new("pure_id").unwrap().fitsh("return 8").unwrap())
            .into_sto();
        let entry_sto = Contract::new()
            .lib(contract_target.to_addr())
            .func(
                Func::new("jump_readonly")
                    .unwrap()
                    .fitsh(
                        r##"
                        lib Dep = 0
                        assert Dep:view_id() == 7
                        assert Dep::pure_id() == 8
                        return 0
                        "##,
                    )
                    .unwrap(),
            )
            .into_sto();

        let mut ext_state = StateMem::default();
        {
            let mut vmsta = crate::VMState::wrap(&mut ext_state);
            vmsta.contract_set_sync_edition(&contract_target, &target_sto);
            vmsta.contract_set_sync_edition(&contract_entry, &entry_sto);
        }

        let script = r##"
            lib C = 0
            codecall C.jump_readonly
        "##;
        let res = run_p2sh_script(
            Address::create_scriptmh([24u8; 20]),
            vec![contract_entry],
            ext_state,
            script,
        );
        assert!(
            res.is_ok(),
            "P2SH codecall should still allow nested view/pure calls: {res:?}"
        );
    }

    #[test]
    fn codecall_uses_explicit_argv_and_allows_nested_calls() {
        let base_addr = test_base_addr();
        let contract_target = test_contract(&base_addr, 23);
        let target_sto = Contract::new()
            .lib(contract_target.to_addr())
            .func(
                Func::new("need_arg")
                    .unwrap()
                    .types(Some(VT::U8), vec![VT::U8])
                    .fitsh(
                        "param { x }
return x
end",
                    )
                    .unwrap(),
            )
            .func(Func::new("leaf").unwrap().fitsh("return 7").unwrap())
            .func(
                Func::new("nested")
                    .unwrap()
                    .fitsh("return this.leaf()")
                    .unwrap(),
            )
            .func(
                Func::new("jump_need_arg")
                    .unwrap()
                    .external()
                    .types(Some(VT::U8), vec![])
                    .fitsh(
                        r##"
                        lib C = 0
                        codecall C.need_arg(9)
                        end
                        "##,
                    )
                    .unwrap(),
            )
            .func(
                Func::new("jump_nested")
                    .unwrap()
                    .external()
                    .fitsh(
                        r##"
                        lib C = 0
                        codecall C.nested
                        end
                        "##,
                    )
                    .unwrap(),
            )
            .into_sto();
        let mut ext_state = StateMem::default();
        {
            let mut vmsta = crate::VMState::wrap(&mut ext_state);
            vmsta.contract_set_sync_edition(&contract_target, &target_sto);
        }

        let arg_script = r##"
            lib C = 0
            assert C.jump_need_arg() == 9
            return 0
        "##;
        let nested_script = r##"
            lib C = 0
            assert C.jump_nested() == 7
            return 0
        "##;

        let arg_res = run_main_script(
            base_addr.clone(),
            vec![contract_target.clone()],
            ext_state.clone(),
            arg_script,
        );
        assert!(
            arg_res.is_ok(),
            "codecall should use explicit argv expression: {arg_res:?}"
        );

        let nested_res =
            run_main_script(base_addr, vec![contract_target], ext_state, nested_script);
        assert!(
            nested_res.is_ok(),
            "codecall should allow nested calls: {nested_res:?}"
        );
    }

    #[test]
    fn codecall_without_caller_ret_contract_ignores_callee_ret_contract() {
        let base_addr = test_base_addr();
        let contract_target = test_contract(&base_addr, 33);
        let target_sto = Contract::new()
            .func(
                Func::new("ret_mismatch")
                    .unwrap()
                    .types(Some(VT::Address), vec![])
                    .fitsh("return 0")
                    .unwrap(),
            )
            .into_sto();
        let mut ext_state = StateMem::default();
        {
            let mut vmsta = crate::VMState::wrap(&mut ext_state);
            vmsta.contract_set_sync_edition(&contract_target, &target_sto);
        }

        let script = r##"
            lib C = 0
            codecall C.ret_mismatch
        "##;
        let rv = run_main_script(base_addr, vec![contract_target], ext_state, script)
            .expect("codecall should follow caller(no contract) return policy");
        assert_eq!(rv, Value::U8(0));
    }

    #[test]
    fn nested_codecall_preserves_outer_caller_return_contract() {
        let base_addr = test_base_addr();
        let contract_outer = test_contract(&base_addr, 34);
        let contract_middle = test_contract(&base_addr, 35);
        let contract_leaf = test_contract(&base_addr, 36);

        let leaf_sto = Contract::new()
            .func(
                Func::new("leaf")
                    .unwrap()
                    .types(Some(VT::U8), vec![])
                    .fitsh("return 7")
                    .unwrap(),
            )
            .into_sto();
        let middle_sto = Contract::new()
            .lib(contract_leaf.to_addr())
            .func(
                Func::new("middle")
                    .unwrap()
                    .types(Some(VT::Bool), vec![])
                    .fitsh(
                        r##"
                        lib L = 0
                        codecall L.leaf
                        "##,
                    )
                    .unwrap(),
            )
            .into_sto();
        let outer_sto = Contract::new()
            .lib(contract_middle.to_addr())
            .func(
                Func::new("outer")
                    .unwrap()
                    .external()
                    .types(Some(VT::U8), vec![])
                    .fitsh(
                        r##"
                        lib M = 0
                        codecall M.middle
                        "##,
                    )
                    .unwrap(),
            )
            .into_sto();

        let mut ext_state = StateMem::default();
        {
            let mut vmsta = crate::VMState::wrap(&mut ext_state);
            vmsta.contract_set_sync_edition(&contract_leaf, &leaf_sto);
            vmsta.contract_set_sync_edition(&contract_middle, &middle_sto);
            vmsta.contract_set_sync_edition(&contract_outer, &outer_sto);
        }

        let script = r##"
            lib O = 0
            assert O.outer() == 7
            return 0
        "##;
        let res = run_main_script(base_addr, vec![contract_outer], ext_state, script);
        assert!(
            res.is_ok(),
            "nested codecall must keep outer caller return contract: {res:?}"
        );
    }

    #[test]
    fn tail_call_unwind_uses_frozen_caller_return_contract() {
        let base_addr = test_base_addr();
        let contract_outer = test_contract(&base_addr, 37);
        let contract_middle = test_contract(&base_addr, 38);
        let contract_leaf = test_contract(&base_addr, 39);

        let leaf_sto = Contract::new()
            .func(
                Func::new("leaf")
                    .unwrap()
                    .external()
                    .types(Some(VT::U8), vec![])
                    .fitsh("return 7")
                    .unwrap(),
            )
            .into_sto();
        let mut middle_codes = vec![crate::rt::Bytecode::CALLEXT as u8, 0];
        middle_codes.extend_from_slice(&crate::rt::calc_func_sign("leaf"));
        let middle_sto = Contract::new()
            .lib(contract_leaf.to_addr())
            .func(
                Func::new("middle")
                    .unwrap()
                    .types(Some(VT::Bool), vec![])
                    .bytecode(middle_codes)
                    .unwrap(),
            )
            .into_sto();
        let outer_sto = Contract::new()
            .lib(contract_middle.to_addr())
            .func(
                Func::new("outer")
                    .unwrap()
                    .external()
                    .types(Some(VT::U8), vec![])
                    .fitsh(
                        r##"
                        lib M = 0
                        codecall M.middle
                        "##,
                    )
                    .unwrap(),
            )
            .into_sto();

        let mut ext_state = StateMem::default();
        {
            let mut vmsta = crate::VMState::wrap(&mut ext_state);
            vmsta.contract_set_sync_edition(&contract_leaf, &leaf_sto);
            vmsta.contract_set_sync_edition(&contract_middle, &middle_sto);
            vmsta.contract_set_sync_edition(&contract_outer, &outer_sto);
        }

        let script = r##"
            lib O = 0
            assert O.outer() == 7
            return 0
        "##;
        let res = run_main_script(base_addr, vec![contract_outer], ext_state, script);
        assert!(
            res.is_ok(),
            "tail unwind must keep frozen caller return contract: {res:?}"
        );
    }

    #[test]
    fn callsuper_uses_direct_parent_order() {
        let base_addr = test_base_addr();
        let contract_a = test_contract(&base_addr, 24);
        let contract_b = test_contract(&base_addr, 25);
        let contract_child = test_contract(&base_addr, 26);

        let a_sto = Contract::new()
            .func(Func::new("f").unwrap().fitsh("return 10").unwrap())
            .into_sto();
        let b_sto = Contract::new()
            .func(Func::new("f").unwrap().fitsh("return 20").unwrap())
            .into_sto();
        let child_sto = Contract::new()
            .inh(contract_a.to_addr())
            .inh(contract_b.to_addr())
            .func(
                Func::new("run")
                    .unwrap()
                    .external()
                    .fitsh(
                        r##"
                        assert super.f() == 10
                        return 0
                        "##,
                    )
                    .unwrap(),
            )
            .into_sto();
        let mut ext_state = StateMem::default();
        {
            let mut vmsta = crate::VMState::wrap(&mut ext_state);
            vmsta.contract_set_sync_edition(&contract_a, &a_sto);
            vmsta.contract_set_sync_edition(&contract_b, &b_sto);
            vmsta.contract_set_sync_edition(&contract_child, &child_sto);
        }

        let script = r##"
            lib C = 0
            return C.run()
        "##;
        let res = run_main_script(base_addr, vec![contract_child], ext_state, script);
        assert!(
            res.is_ok(),
            "super should choose first direct parent in inherit order"
        );
    }

    #[test]
    fn callsuper_never_resolves_back_to_current_owner() {
        let base_addr = test_base_addr();
        let contract_parent = test_contract(&base_addr, 41);
        let contract_child = test_contract(&base_addr, 42);

        let parent_sto = Contract::new().inh(contract_child.to_addr()).into_sto();
        let child_sto = Contract::new()
            .inh(contract_parent.to_addr())
            .func(Func::new("f").unwrap().fitsh("return 7").unwrap())
            .func(
                Func::new("run")
                    .unwrap()
                    .external()
                    .fitsh(
                        r##"
                        let _ = super.f()
                        return 0
                        "##,
                    )
                    .unwrap(),
            )
            .into_sto();

        let mut ext_state = StateMem::default();
        {
            let mut vmsta = crate::VMState::wrap(&mut ext_state);
            vmsta.contract_set_sync_edition(&contract_parent, &parent_sto);
            vmsta.contract_set_sync_edition(&contract_child, &child_sto);
        }

        let script = r##"
            lib C = 0
            return C.run()
        "##;
        let res = run_main_script(base_addr, vec![contract_child], ext_state, script);
        assert_err_contains(res, "CallNotExist");
    }

    #[test]
    fn callextview_callusepure_and_codecall_local_lookup_positive_paths() {
        let base_addr = test_base_addr();
        let contract_target = test_contract(&base_addr, 27);
        let target_sto = Contract::new()
            .func(Func::new("view_ok").unwrap().fitsh("return 7").unwrap())
            .func(Func::new("pure_ok").unwrap().fitsh("return 8").unwrap())
            .func(Func::new("code_ok").unwrap().fitsh("return 0").unwrap())
            .func(
                Func::new("self_ok")
                    .unwrap()
                    .external()
                    .fitsh(
                        r##"
                        assert self:view_ok() == 7
                        assert self::pure_ok() == 8
                        return 0
                        "##,
                    )
                    .unwrap(),
            )
            .into_sto();
        let mut ext_state = StateMem::default();
        {
            let mut vmsta = crate::VMState::wrap(&mut ext_state);
            vmsta.contract_set_sync_edition(&contract_target, &target_sto);
        }

        let script = r##"
            lib C = 0
            assert C:view_ok() == 7
            assert C::pure_ok() == 8
            assert C.self_ok() == 0
            codecall C.code_ok
            end
        "##;
        let res = run_main_script(base_addr, vec![contract_target], ext_state, script);
        assert!(
            res.is_ok(),
            "local lookup should succeed for ext-view/use-pure/codecall/self shortcuts"
        );
    }

    #[test]
    fn newframe_calls_rebind_current_libctx_for_callee_lib_calls() {
        let base_addr = test_base_addr();
        let contract_entry = test_contract(&base_addr, 71);
        let contract_entry_lib = test_contract(&base_addr, 72);
        let contract_code_lib = test_contract(&base_addr, 73);

        let entry_lib_sto = Contract::new()
            .func(
                Func::new("id")
                    .unwrap()
                    .external()
                    .fitsh("return 20")
                    .unwrap(),
            )
            .func(Func::new("view_id").unwrap().fitsh("return 21").unwrap())
            .func(Func::new("pure_id").unwrap().fitsh("return 22").unwrap())
            .into_sto();
        let code_lib_sto = Contract::new()
            .func(
                Func::new("id")
                    .unwrap()
                    .external()
                    .fitsh("return 30")
                    .unwrap(),
            )
            .func(Func::new("view_id").unwrap().fitsh("return 31").unwrap())
            .func(Func::new("pure_id").unwrap().fitsh("return 32").unwrap())
            .into_sto();
        let entry_sto = Contract::new()
            .lib(contract_entry_lib.to_addr())
            .lib(contract_code_lib.to_addr())
            .func(
                Func::new("ext_probe")
                    .unwrap()
                    .external()
                    .fitsh(
                        r##"
                        lib Dep = 1
                        assert Dep.id() == 30
                        return 0
                        "##,
                    )
                    .unwrap(),
            )
            .func(
                Func::new("view_probe")
                    .unwrap()
                    .external()
                    .fitsh(
                        r##"
                        lib Dep = 1
                        assert Dep:view_id() == 31
                        return 0
                        "##,
                    )
                    .unwrap(),
            )
            .func(
                Func::new("pure_probe")
                    .unwrap()
                    .external()
                    .fitsh(
                        r##"
                        lib Dep = 1
                        assert Dep::pure_id() == 32
                        return 0
                        "##,
                    )
                    .unwrap(),
            )
            .into_sto();

        let mut ext_state = StateMem::default();
        {
            let mut vmsta = crate::VMState::wrap(&mut ext_state);
            vmsta.contract_set_sync_edition(&contract_entry_lib, &entry_lib_sto);
            vmsta.contract_set_sync_edition(&contract_code_lib, &code_lib_sto);
            vmsta.contract_set_sync_edition(&contract_entry, &entry_sto);
        }

        let script = r##"
            lib C = 0
            assert C.ext_probe() == 0
            assert C:view_probe() == 0
            assert C::pure_probe() == 0
            return 0
        "##;
        let res = run_main_script(
            base_addr,
            vec![contract_entry, contract_entry_lib],
            ext_state,
            script,
        );
        assert!(
            res.is_ok(),
            "new-frame calls should resolve callee lib lookups on code_owner: {res:?}"
        );
    }

    #[test]
    fn codecall_rebinds_callee_libs_for_nested_calls() {
        let base_addr = test_base_addr();
        let contract_entry = test_contract(&base_addr, 81);
        let contract_entry_lib = test_contract(&base_addr, 82);
        let contract_code_lib = test_contract(&base_addr, 83);

        let entry_lib_sto = Contract::new()
            .func(
                Func::new("id")
                    .unwrap()
                    .external()
                    .fitsh("return 20")
                    .unwrap(),
            )
            .func(Func::new("view_id").unwrap().fitsh("return 21").unwrap())
            .func(Func::new("pure_id").unwrap().fitsh("return 22").unwrap())
            .into_sto();
        let code_lib_sto = Contract::new()
            .func(
                Func::new("id")
                    .unwrap()
                    .external()
                    .fitsh("return 30")
                    .unwrap(),
            )
            .func(Func::new("view_id").unwrap().fitsh("return 31").unwrap())
            .func(Func::new("pure_id").unwrap().fitsh("return 32").unwrap())
            .func(Func::new("code_ok").unwrap().fitsh("return 0").unwrap())
            .into_sto();
        let entry_sto = Contract::new()
            .lib(contract_entry_lib.to_addr())
            .lib(contract_code_lib.to_addr())
            .func(
                Func::new("jump_ext")
                    .unwrap()
                    .external()
                    .fitsh(
                        r##"
                        lib Dep = 1
                        assert Dep.id() == 30
                        return 0
                        "##,
                    )
                    .unwrap(),
            )
            .func(
                Func::new("jump_view")
                    .unwrap()
                    .external()
                    .fitsh(
                        r##"
                        lib Dep = 1
                        assert Dep:view_id() == 31
                        return 0
                        "##,
                    )
                    .unwrap(),
            )
            .func(
                Func::new("jump_pure")
                    .unwrap()
                    .external()
                    .fitsh(
                        r##"
                        lib Dep = 1
                        assert Dep::pure_id() == 32
                        return 0
                        "##,
                    )
                    .unwrap(),
            )
            .func(
                Func::new("jump_code")
                    .unwrap()
                    .external()
                    .fitsh(
                        r##"
                        lib Dep = 1
                        codecall Dep.code_ok
                        "##,
                    )
                    .unwrap(),
            )
            .into_sto();

        let mut ext_state = StateMem::default();
        {
            let mut vmsta = crate::VMState::wrap(&mut ext_state);
            vmsta.contract_set_sync_edition(&contract_entry_lib, &entry_lib_sto);
            vmsta.contract_set_sync_edition(&contract_code_lib, &code_lib_sto);
            vmsta.contract_set_sync_edition(&contract_entry, &entry_sto);
        }

        let tx_libs = vec![contract_entry.clone(), contract_entry_lib.clone()];
        let run_codecall = |func: &str| -> Ret<Value> {
            let script = format!(
                r##"
                lib C = 0
                codecall C.{func}
                "##,
            );
            run_main_script(
                base_addr.clone(),
                tx_libs.clone(),
                ext_state.clone(),
                &script,
            )
        };
        for func in ["jump_ext", "jump_view", "jump_pure", "jump_code"] {
            let res = run_codecall(func);
            assert!(
                res.is_ok(),
                "codecall should rebind callee libs for {func}: {res:?}"
            );
        }
    }

    #[test]
    fn call_depth_overflow_fails_before_loading_target_contract() {
        let base_addr = test_base_addr();
        let contract_entry = test_contract(&base_addr, 84);
        let contract_target = test_contract(&base_addr, 85);

        let target_sto = Contract::new()
            .func(
                Func::new("probe")
                    .unwrap()
                    .external()
                    .fitsh("return 7")
                    .unwrap(),
            )
            .into_sto();
        let entry_sto = Contract::new()
            .lib(contract_target.to_addr())
            .func(
                Func::new("deep")
                    .unwrap()
                    .external()
                    .fitsh(
                        r##"
                        lib T = 0
                        return T.probe()
                        "##,
                    )
                    .unwrap(),
            )
            .into_sto();

        let mut ext_state = StateMem::default();
        {
            let mut vmsta = crate::VMState::wrap(&mut ext_state);
            vmsta.contract_set_sync_edition(&contract_entry, &entry_sto);
            vmsta.contract_set_sync_edition(&contract_target, &target_sto);
        }

        let main_codes = lang_to_bytecode(
            r##"
            lib E = 0
            return E.deep()
            "##,
        )
        .unwrap();
        let mut env = Env::default();
        env.block.height = 1;
        env.tx.main = base_addr;
        env.tx.addrs = vec![contract_entry.clone().into_addr()];

        let tx = StubTxBuilder::new()
            .ty(0)
            .main(base_addr)
            .addrs(env.tx.addrs.clone())
            .fee(Amount::zero())
            .gas_max(1)
            .tx_size(128)
            .fee_purity(1)
            .build();
        let mut ctx = make_ctx_with_state(env, Box::new(ext_state), &tx);
        let mut gas: i64 = 1_000_000;
        let mut exec = crate::frame::ExecEnv {
            ctx: &mut ctx as &mut dyn Context,
            gas: &mut gas,
        };
        let mut machine = Machine::create(Resoure::create(1));
        machine.r.space_cap.call_depth = 1;

        let err = machine
            .main_call(&mut exec, CodeType::Bytecode, main_codes.into())
            .expect_err("nested call must exceed call_depth limit");
        assert!(err.contains("OutOfCallDepth"), "unexpected error: {err}");
        assert!(machine.r.contracts.contains_key(&contract_entry));
        assert!(!machine.r.contracts.contains_key(&contract_target));
    }

    #[test]
    fn user_call_missing_argv_fails_before_loading_target_contract() {
        use crate::rt::Bytecode;

        let base_addr = test_base_addr();
        let contract_entry = test_contract(&base_addr, 86);
        let contract_target = test_contract(&base_addr, 87);

        let target_sto = Contract::new()
            .func(
                Func::new("probe")
                    .unwrap()
                    .external()
                    .fitsh("return 7")
                    .unwrap(),
            )
            .into_sto();
        let mut entry_codes = vec![Bytecode::POP as u8, Bytecode::CALLEXT as u8, 0];
        entry_codes.extend_from_slice(&crate::rt::calc_func_sign("probe"));
        let entry_sto = Contract::new()
            .lib(contract_target.to_addr())
            .func(
                Func::new("deep")
                    .unwrap()
                    .external()
                    .bytecode(entry_codes)
                    .unwrap(),
            )
            .into_sto();

        let mut ext_state = StateMem::default();
        {
            let mut vmsta = crate::VMState::wrap(&mut ext_state);
            vmsta.contract_set_sync_edition(&contract_entry, &entry_sto);
            vmsta.contract_set_sync_edition(&contract_target, &target_sto);
        }

        let main_codes = lang_to_bytecode(
            r##"
            lib E = 0
            return E.deep()
            "##,
        )
        .unwrap();
        let mut env = Env::default();
        env.block.height = 1;
        env.tx.main = base_addr;
        env.tx.addrs = vec![contract_entry.clone().into_addr()];

        let tx = StubTxBuilder::new()
            .ty(0)
            .main(base_addr)
            .addrs(env.tx.addrs.clone())
            .fee(Amount::zero())
            .gas_max(1)
            .tx_size(128)
            .fee_purity(1)
            .build();
        let mut ctx = make_ctx_with_state(env, Box::new(ext_state), &tx);
        let mut gas: i64 = 1_000_000;
        let mut exec = crate::frame::ExecEnv {
            ctx: &mut ctx as &mut dyn Context,
            gas: &mut gas,
        };
        let mut machine = Machine::create(Resoure::create(1));

        let err = machine
            .main_call(&mut exec, CodeType::Bytecode, main_codes.into())
            .expect_err("nested call without argv must fail locally");
        assert!(err.contains("Read empty stack"), "unexpected error: {err}");
        assert!(machine.r.contracts.contains_key(&contract_entry));
        assert!(!machine.r.contracts.contains_key(&contract_target));
    }

    #[test]
    fn abst_call_invalid_param_fails_before_loading_target_contract() {
        let base_addr = test_base_addr();
        let contract_target = test_contract(&base_addr, 88);

        let target_sto = Contract::new()
            .syst(Abst::new(AbstCall::Construct).fitsh("return 0").unwrap())
            .into_sto();

        let mut ext_state = StateMem::default();
        {
            let mut vmsta = crate::VMState::wrap(&mut ext_state);
            vmsta.contract_set_sync_edition(&contract_target, &target_sto);
        }

        let mut env = Env::default();
        env.block.height = 1;
        env.tx.main = base_addr;

        let tx = StubTxBuilder::new()
            .ty(0)
            .main(base_addr)
            .fee(Amount::zero())
            .gas_max(1)
            .tx_size(128)
            .fee_purity(1)
            .build();
        let mut ctx = make_ctx_with_state(env, Box::new(ext_state), &tx);
        let mut gas: i64 = 1_000_000;
        let mut exec = crate::frame::ExecEnv {
            ctx: &mut ctx as &mut dyn Context,
            gas: &mut gas,
        };
        let mut machine = Machine::create(Resoure::create(1));

        let err = machine
            .abst_call(
                &mut exec,
                AbstCall::Construct,
                contract_target.clone(),
                Value::HeapSlice((0, 1)),
            )
            .expect_err("invalid abst argv must fail locally");
        assert!(err.contains("CastBeFnArgvFail"), "unexpected error: {err}");
        assert!(!machine.r.contracts.contains_key(&contract_target));
    }

    #[test]
    fn min_call_gas_is_consumed_from_shared_counter() {
        use crate::rt::Bytecode;

        // Prepare a state with enough HAC for gas spending.
        let main = Address::from_readable("1MzNY1oA3kfgYi75zquj3SRUPYztzXHzK9").unwrap();
        let mut env = Env::default();
        env.block.height = 1;
        env.tx.main = main;
        env.tx.addrs = vec![main];

        let tx = StubTxBuilder::new()
            .ty(TransactionType3::TYPE)
            .main(main)
            .addrs(vec![main])
            .fee(Amount::unit238(10_000_000))
            .gas_max(17)
            .tx_size(128)
            .fee_purity(3200)
            .build();
        let mut ctx = make_ctx_with_state(env, Box::new(StateMem::default()), &tx);
        protocol::operate::hac_add(&mut ctx, &main, &Amount::unit238(1_000_000_000)).unwrap();
        ctx.gas_init_tx(decode_gas_budget(17), 1).unwrap();

        let mut vm = MachineBox::new(Machine::create(Resoure::create(1)));
        // END is a minimal "return nil" program; actual instruction gas is tiny.
        let codes = vec![Bytecode::END as u8];

        vm.call(VMCall::new(
            &mut ctx,
            EntryKind::Main as u8,
            CodeType::Bytecode as u8,
            codes.clone().into(),
            Box::new(Value::Nil),
        ))
        .unwrap();

        let gsext = GasExtra::new(1);
        let min = gsext.main_call_min;
        let budget = decode_gas_budget(17); // lookup-table decoded budget
                                            // The min-call cost must be reflected in the shared gas counter.
        assert!(
            ctx.gas_remaining() <= (budget - min),
            "ctx gas remaining should include min cost deduction"
        );
    }

    #[test]
    fn snapshot_restore_volatile_fields_only_except_gas_and_warmups() {
        use crate::rt::Bytecode;
        use std::sync::Arc;

        let main = Address::from_readable("1MzNY1oA3kfgYi75zquj3SRUPYztzXHzK9").unwrap();
        let mut env = Env::default();
        env.block.height = 1;
        env.tx.main = main;
        env.tx.addrs = vec![main];

        let tx = StubTxBuilder::new()
            .ty(TransactionType3::TYPE)
            .main(main)
            .addrs(vec![main])
            .fee(Amount::unit238(10_000_000))
            .gas_max(17)
            .tx_size(128)
            .fee_purity(1)
            .build();
        let mut ctx = make_ctx_with_state(env, Box::new(StateMem::default()), &tx);
        protocol::operate::hac_add(&mut ctx, &main, &Amount::unit238(1_000_000_000)).unwrap();
        ctx.gas_init_tx(decode_gas_budget(17), 1).unwrap();

        let mut vm = MachineBox::new(Machine::create(Resoure::create(1)));
        let codes = vec![Bytecode::END as u8];
        vm.call(VMCall::new(
            &mut ctx,
            EntryKind::Main as u8,
            CodeType::Bytecode as u8,
            codes.clone().into(),
            Box::new(Value::Nil),
        ))
        .unwrap();

        let warm_a = test_contract(&main, 201);
        let warm_b = test_contract(&main, 202);
        vm.machine
            .as_mut()
            .unwrap()
            .r
            .contracts
            .insert(warm_a.clone(), Arc::new(ContractObj::default()));
        let snap = vm.snapshot_volatile();

        // Mutate gas remaining in context (outside VM volatile snapshot).
        *ctx.vm_gas_mut().unwrap().gas_remaining_mut().unwrap() = 1;
        // Mutate volatile fields (should be restored)
        vm.machine
            .as_mut()
            .unwrap()
            .r
            .contracts
            .insert(warm_b.clone(), Arc::new(ContractObj::default()));

        vm.restore_volatile(snap);

        // Gas remaining is NOT restored: gas usage must stay monotonic in one tx.
        assert_eq!(ctx.gas_remaining(), 1);
        // Warmup accounting is NOT restored: gas-charged preload state must remain monotonic.
        assert_eq!(vm.machine.as_ref().unwrap().r.contracts.len(), 2);
        assert!(vm
            .machine
            .as_ref()
            .unwrap()
            .r
            .contracts
            .contains_key(&warm_b));
    }

    #[test]
    fn low_fee_does_not_panic_in_settle() {
        use crate::rt::Bytecode;

        let main = Address::from_readable("1MzNY1oA3kfgYi75zquj3SRUPYztzXHzK9").unwrap();
        let mut env = Env::default();
        env.block.height = 1;
        env.tx.main = main;
        env.tx.addrs = vec![main];

        let tx = StubTxBuilder::new()
            .ty(TransactionType3::TYPE)
            .main(main)
            .addrs(vec![main])
            .fee(Amount::unit238(1))
            .gas_max(1)
            .tx_size(128)
            .fee_purity(1)
            .build();
        let mut ctx = make_ctx_with_state(env, Box::new(StateMem::default()), &tx);
        protocol::operate::hac_add(&mut ctx, &main, &Amount::unit238(1_000_000_000)).unwrap();
        ctx.gas_init_tx(decode_gas_budget(1), 1).unwrap();

        let mut vm = MachineBox::new(Machine::create(Resoure::create(1)));
        let codes = vec![Bytecode::END as u8];

        let res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            vm.call(VMCall::new(
                &mut ctx,
                EntryKind::Main as u8,
                CodeType::Bytecode as u8,
                codes.clone().into(),
                Box::new(Value::Nil),
            ))
        }));
        assert!(res.is_ok(), "settle must not panic");
        // low fee may cause an error return, but must never panic
        let _ = res.unwrap();
    }

    #[test]
    fn reentry_level_restores_after_early_return() {
        use crate::rt::Bytecode;

        let main = Address::from_readable("1MzNY1oA3kfgYi75zquj3SRUPYztzXHzK9").unwrap();
        let mut env = Env::default();
        env.block.height = 1;
        env.tx.main = main;
        env.tx.addrs = vec![main];

        let tx = StubTxBuilder::new()
            .ty(TransactionType3::TYPE)
            .main(main)
            .addrs(vec![main])
            .fee(Amount::unit238(10_000_000))
            .gas_max(17)
            .tx_size(128)
            .fee_purity(1)
            .build();
        let mut ctx = make_ctx_with_state(env, Box::new(StateMem::default()), &tx);
        protocol::operate::hac_add(&mut ctx, &main, &Amount::unit238(1_000_000_000)).unwrap();
        ctx.gas_init_tx(decode_gas_budget(17), 1).unwrap();

        let mut vm = MachineBox::new(Machine::create(Resoure::create(1)));
        // Invalid code type causes early return inside Main branch before previous manual leave() point.
        let early = vm.call(VMCall::new(
            &mut ctx,
            EntryKind::Main as u8,
            255,
            Arc::from(vec![]),
            Box::new(Value::Nil),
        ));
        assert!(early.is_err(), "invalid code type must fail");
        assert_eq!(
            vm.call_state.reentry_level, 0,
            "re-entry level must be restored after early return"
        );

        // Next normal call should still behave as an outermost call.
        let codes = vec![Bytecode::END as u8];
        let ok = vm.call(VMCall::new(
            &mut ctx,
            EntryKind::Main as u8,
            CodeType::Bytecode as u8,
            codes.into(),
            Box::new(Value::Nil),
        ));
        assert!(
            ok.is_ok(),
            "subsequent call must not be poisoned by previous early return"
        );
        assert_eq!(
            vm.call_state.reentry_level, 0,
            "re-entry level must remain balanced after successful call"
        );
    }

    fn read_hac_balance(ctx: &mut dyn Context, addr: &Address) -> Amount {
        protocol::state::CoreState::wrap(ctx.state())
            .balance(addr)
            .unwrap_or_default()
            .hacash
    }

    #[test]
    fn outermost_failed_call_consumes_remaining_without_burn() {
        let main = Address::from_readable("1MzNY1oA3kfgYi75zquj3SRUPYztzXHzK9").unwrap();
        let mut env = Env::default();
        env.block.height = 1;
        env.tx.main = main;
        env.tx.addrs = vec![main];

        let tx = StubTxBuilder::new()
            .ty(TransactionType3::TYPE)
            .main(main)
            .addrs(vec![main])
            .fee(Amount::unit238(10_000_000))
            .gas_max(17)
            .tx_size(128)
            .fee_purity(3200)
            .build();
        let mut ctx = make_ctx_with_state(env, Box::new(StateMem::default()), &tx);
        protocol::operate::hac_add(&mut ctx, &main, &Amount::unit238(1_000_000_000)).unwrap();
        ctx.gas_init_tx(decode_gas_budget(17), 1).unwrap();

        let mut vm = MachineBox::new(Machine::create(Resoure::create(1)));
        let fail_codes = lang_to_bytecode("return 1").unwrap();

        let bal_before = read_hac_balance(&mut ctx, &main);
        let call = vm.call(VMCall::new(
            &mut ctx,
            EntryKind::Main as u8,
            CodeType::Bytecode as u8,
            fail_codes.into(),
            Box::new(Value::Nil),
        ));
        let bal_after = read_hac_balance(&mut ctx, &main);

        assert!(
            call.is_err(),
            "vm call should fail when script returns non-zero"
        );
        assert!(
            ctx.gas_remaining() < decode_gas_budget(17),
            "remaining gas should decrease even when call fails"
        );
        assert_eq!(
            bal_before, bal_after,
            "outermost failed call currently skips settle burn"
        );
    }

    #[test]
    fn fail_then_success_charges_same_as_success_only() {
        let main = Address::from_readable("1MzNY1oA3kfgYi75zquj3SRUPYztzXHzK9").unwrap();
        let fail_codes = lang_to_bytecode("return 1").unwrap();
        let ok_codes = vec![crate::rt::Bytecode::END as u8];

        let run = |run_failed_first: bool| -> (Amount, i64) {
            let mut env = Env::default();
            env.block.height = 1;
            env.tx.main = main;
            env.tx.addrs = vec![main];

            let tx = StubTxBuilder::new()
                .ty(TransactionType3::TYPE)
                .main(main)
                .addrs(vec![main])
                .fee(Amount::unit238(10_000_000))
                .gas_max(17)
                .tx_size(128)
                .fee_purity(3200)
                .build();
            let mut ctx = make_ctx_with_state(env, Box::new(StateMem::default()), &tx);
            protocol::operate::hac_add(&mut ctx, &main, &Amount::unit238(1_000_000_000)).unwrap();
            ctx.gas_init_tx(decode_gas_budget(17), 1).unwrap();

            let mut vm = MachineBox::new(Machine::create(Resoure::create(1)));
            if run_failed_first {
                let failed = vm.call(VMCall::new(
                    &mut ctx,
                    EntryKind::Main as u8,
                    CodeType::Bytecode as u8,
                    fail_codes.clone().into(),
                    Box::new(Value::Nil),
                ));
                assert!(failed.is_err(), "prelude failed call must fail");
            }

            let ok = vm.call(VMCall::new(
                &mut ctx,
                EntryKind::Main as u8,
                CodeType::Bytecode as u8,
                ok_codes.clone().into(),
                Box::new(Value::Nil),
            ));
            assert!(ok.is_ok(), "final success call must succeed");

            (read_hac_balance(&mut ctx, &main), ctx.gas_remaining())
        };

        let (bal_success_only, rem_success_only) = run(false);
        let (bal_fail_then_success, rem_fail_then_success) = run(true);

        assert_eq!(
            bal_fail_then_success, bal_success_only,
            "failed outermost call before a successful one is currently not additionally charged"
        );
        assert!(
            rem_fail_then_success < rem_success_only,
            "failed call should still consume shared remaining gas"
        );
    }

    /* i64::MAX  = 9223372036854775807 10000 HAC =   10000000000000000:236 0.00000001 = 1:240 = 10000:236 */
}
