#[derive(Clone, Default)]
pub struct VmCallState {
    reentry_level: u32,
}

struct VmReentryGuard<'a> {
    call_state: &'a mut VmCallState,
}

impl<'a> VmReentryGuard<'a> {
    fn enter(call_state: &'a mut VmCallState, max_reentry: u32) -> Ret<Self> {
        call_state.enter(max_reentry)?;
        Ok(Self { call_state })
    }
}

impl Drop for VmReentryGuard<'_> {
    fn drop(&mut self) {
        self.call_state.leave();
    }
}

impl VmCallState {
    fn enter(&mut self, max_reentry: u32) -> Rerr {
        let next_level = self
            .reentry_level
            .checked_add(1)
            .ok_or_else(|| "re-entry level overflow".to_owned())?;
        if next_level > max_reentry + 1 {
            return errf!(
                "re-entry level {} exceeded limit {}",
                next_level - 1,
                max_reentry
            );
        }
        self.reentry_level = next_level;
        Ok(())
    }

    fn leave(&mut self) {
        self.reentry_level = self.reentry_level.saturating_sub(1);
    }
}

pub(crate) enum VmCallReq {
    Main {
        code_type: CodeType,
        codes: Arc<[u8]>,
    },
    P2sh {
        code_type: CodeType,
        state_addr: Address,
        libs: Vec<ContractAddress>,
        codes: ByteView,
        intent_binding: IntentScope,
        param: Value,
    },
    Abst {
        kind: AbstCall,
        contract_addr: ContractAddress,
        intent_binding: IntentScope,
        param: Value,
    },
}

impl VmCallReq {
    fn entry_kind(&self) -> EntryKind {
        match self {
            Self::Main { .. } => EntryKind::Main,
            Self::P2sh { .. } => EntryKind::P2sh,
            Self::Abst { .. } => EntryKind::Abst,
        }
    }

    fn min_call_cost(&self, gas_extra: &GasExtra) -> i64 {
        self.entry_kind().min_call_cost(gas_extra)
    }

    fn execute(self, machine: &mut Machine, ctx: &mut dyn Context) -> XRet<Value> {
        match self {
            Self::Main { code_type, codes } => machine.main_call(ctx, code_type, codes),
            Self::P2sh {
                code_type,
                state_addr,
                libs,
                codes,
                intent_binding,
                param,
            } => machine.p2sh_call(ctx, code_type, state_addr, libs, codes, intent_binding, param),
            Self::Abst {
                kind,
                contract_addr,
                intent_binding,
                param,
            } => machine.abst_call(ctx, kind, contract_addr, intent_binding, param),
        }
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

    #[inline]
    fn machine_ref(&self) -> Ret<&Machine> {
        self.machine
            .as_ref()
            .ok_or_else(|| "machine runtime missing".to_owned())
    }

    #[inline]
    fn machine_mut(&mut self) -> Ret<&mut Machine> {
        self.machine
            .as_mut()
            .ok_or_else(|| "machine runtime missing".to_owned())
    }

    pub fn sandbox_main_call_raw(
        &mut self,
        ctx: &mut dyn Context,
        ctype: CodeType,
        codes: Arc<[u8]>,
    ) -> Ret<Value> {
        let (_, ret_val) = self.sandbox_main_call_raw_with_gas(ctx, ctype, codes)?;
        Ok(ret_val)
    }

    pub fn sandbox_main_call_raw_with_gas(
        &mut self,
        ctx: &mut dyn Context,
        ctype: CodeType,
        codes: Arc<[u8]>,
    ) -> Ret<(GasUse, Value)> {
        let Some(machine) = self.machine.as_ref() else {
            return errf!("machine runtime missing");
        };
        let max_reentry = machine.r.space_cap.reentry_level;
        let _guard = VmReentryGuard::enter(&mut self.call_state, max_reentry)
            .map_err(|e| e.to_string())?;
        let call_level = _guard.call_state.reentry_level;
        let min_cost = EntryKind::Main.min_call_cost(&machine.r.gas_extra);
        let gas_before = ctx.gas_remaining();
        let call_base = {
            let Some(machine) = self.machine.as_mut() else {
                return errf!("machine runtime missing");
            };
            if call_level <= 1 {
                machine.r.reset_call_gas_use();
                GasUse::default()
            } else {
                machine.r.gas_use()
            }
        };
        let result = {
            let Some(machine) = self.machine.as_mut() else {
                return errf!("machine runtime missing");
            };
            machine.main_call_raw(ctx, ctype, codes)
        };
        let gas_after = ctx.gas_remaining();
        let actual = gas_before - gas_after;
        let Some(machine) = self.machine.as_mut() else {
            return errf!("machine runtime missing");
        };
        if actual < min_cost {
            let delta = min_cost - actual;
            let next_compute = machine.r.next_compute_used(delta).map_err(|e| e.to_string())?;
            ctx.gas_charge(delta)?;
            machine.r.gas_use.compute = next_compute;
        }
        let total_cost = machine.r.gas_use();
        let Some(cost) = total_cost.checked_sub(call_base) else {
            return errf!("gas cost underflow: total={:?}, base={:?}", total_cost, call_base);
        };
        let ret_val = result?;
        if cost.total() <= 0 {
            return errf!("gas cost invalid: {}", cost.total());
        }
        Ok((cost, ret_val))
    }

    fn execute_req_internal(
        &mut self,
        ctx: &mut dyn Context,
        req: VmCallReq,
    ) -> XRet<(GasUse, Value)> {
        let Some(machine) = self.machine.as_ref() else {
            return xerrf!("machine runtime missing");
        };
        let max_reentry = machine.r.space_cap.reentry_level;
        let _guard = VmReentryGuard::enter(&mut self.call_state, max_reentry)?;
        let call_level = _guard.call_state.reentry_level;
        let min_cost = req.min_call_cost(&machine.r.gas_extra);
        let gas_before = ctx.gas_remaining();
        let old_exec_from = ctx.exec_from();
        ctx.exec_from_set(basis::component::ExecFrom::Call);
        let (call_base, result) = {
            let Some(machine) = self.machine.as_mut() else {
                ctx.exec_from_set(old_exec_from);
                return xerrf!("machine runtime missing");
            };
            let call_base = if call_level <= 1 {
                machine.r.reset_call_gas_use();
                GasUse::default()
            } else {
                machine.r.gas_use()
            };
            let result = req.execute(machine, ctx);
            (call_base, result)
        };
        ctx.exec_from_set(old_exec_from);
        let gas_after = ctx.gas_remaining();
        let actual = gas_before - gas_after;
        let Some(machine) = self.machine.as_mut() else {
            return xerrf!("machine runtime missing");
        };
        if actual < min_cost {
            let delta = min_cost - actual;
            let next_compute = machine.r.next_compute_used(delta)?;
            ctx.gas_charge(delta)?;
            machine.r.gas_use.compute = next_compute;
        }
        let total_cost = machine.r.gas_use();
        let Some(cost) = total_cost.checked_sub(call_base) else {
            return xerrf!(
                "gas cost underflow: total={:?}, base={:?}",
                total_cost,
                call_base
            );
        };
        let resv = result?;
        if cost.total() <= 0 {
            return xerrf!("gas cost invalid: {}", cost.total());
        }
        Ok((cost, resv))
    }
}

impl VM for MachineBox {
    fn current_intent_scope(&mut self) -> Option<Option<u64>> {
        self.machine_ref().ok().and_then(Machine::current_intent_scope)
    }

    fn snapshot_volatile(&mut self) -> Box<dyn Any> {
        match self.machine_ref() {
            Ok(m) => Box::new((
                m.r.global_map.clone(),
                m.r.memory_map.clone(),
                m.r.intents.clone(),
                m.r.deferred_registry.clone(),
            )),
            Err(_) => Box::new((
                GKVMap::default(),
                CtcKVMap::default(),
                IntentRuntime::default(),
                DeferredRegistry::default(),
            )),
        }
    }

    fn restore_volatile(&mut self, snap: Box<dyn Any>) {
        let Ok(snap) = snap.downcast::<(GKVMap, CtcKVMap, IntentRuntime, DeferredRegistry)>() else {
            return;
        };
        let (global_map, memory_map, intents, deferred_registry) = *snap;
        if let Ok(m) = self.machine_mut() {
            m.r.global_map = global_map;
            m.r.memory_map = memory_map;
            m.r.intents = intents;
            m.r.deferred_registry = deferred_registry;
        }
    }

    fn restore_but_keep_warmup(&mut self) {
        if let Ok(m) = self.machine_mut() {
            m.r.global_map.clear();
            m.r.memory_map.clear();
            m.r.intents.clear();
            m.r.deferred_registry.clear();
        }
    }

    fn invalidate_contract_cache(&mut self, addr: &Address) {
        let Ok(caddr) = ContractAddress::from_addr(*addr) else {
            return;
        };
        if let Ok(m) = self.machine_mut() {
            m.r.contracts.remove(&caddr);
        }
        global_machine_manager()
            .contract_cache()
            .remove_addr(&caddr);
    }

    fn drain_deferred(&mut self, ctx: &mut dyn Context) -> Rerr {
        let callbacks = {
            let m = self.machine_mut().map_err(|e| e.to_string())?;
            m.r.deferred_registry.drain_lifo()
        };
        for caddr in callbacks {
            let _ = self
                .execute_req_internal(
                    ctx,
                    VmCallReq::Abst {
                        kind: AbstCall::Deferred,
                        contract_addr: caddr.addr,
                        intent_binding: Some(caddr.intent_id),
                        param: Value::Nil,
                    },
                )
                .map_err(|e| e.to_string())?;
        }
        Ok(())
    }

    fn call(&mut self, ctx: &mut dyn Context, req: Box<dyn Any>) -> XRet<(GasUse, Box<dyn Any>)> {
        let Ok(req) = req.downcast::<VmCallReq>() else {
            return xerrf!("vm call request type mismatch");
        };
        let (cost, resv) = self.execute_req_internal(ctx, *req)?;
        Ok((cost, Box::new(resv)))
    }
}

/*********************************/

#[allow(dead_code)]
pub struct Machine {
    r: Resoure,
    frames: Vec<CallFrame>,
}

impl Machine {
    fn current_intent_scope(&self) -> Option<Option<u64>> {
        self.frames
            .last()
            .and_then(CallFrame::current_intent_scope)
    }

    pub fn is_in_calling(&self) -> bool {
        !self.frames.is_empty()
    }

    pub fn create(r: Resoure) -> Self {
        Self { r, frames: vec![] }
    }

    pub fn main_call_raw<H: VmHost + ?Sized>(
        &mut self,
        host: &mut H,
        ctype: CodeType,
        codes: Arc<[u8]>,
    ) -> Ret<Value> {
        // Caller must pre-validate code bytes. Production entry actions run convert_and_check before setup_vm_run.
        let fnobj = FnObj::plain(ctype, codes, 0, None);
        Ok(self.do_call(
            host,
            EntryKind::Main.root_exec(),
            &fnobj,
            host.main_entry_bindings(),
            None,
        )?)
    }

    pub fn main_call<H: VmHost + ?Sized>(
        &mut self,
        host: &mut H,
        ctype: CodeType,
        codes: Arc<[u8]>,
    ) -> XRet<Value> {
        let rv = self.main_call_raw(host, ctype, codes)?;
        check_vm_return_value(&rv, "main call")?;
        Ok(rv)
    }

    pub fn abst_call<H: VmHost + ?Sized>(
        &mut self,
        host: &mut H,
        cty: AbstCall,
        contract_addr: ContractAddress,
        intent_binding: IntentScope,
        param: Value,
    ) -> XRet<Value> {
        let exec = EntryKind::Abst.root_exec();
        exec.ensure_call_depth(&self.r.space_cap).map_err(XError::from)?;
        param.check_func_argv().map_err(XError::from)?;
        param
            .check_container_cap(&self.r.space_cap)
            .map_err(XError::from)?;
        let adr = contract_addr.to_readable();
        let Some(hit) = self
            .r
            .resolve_abstfn(host, &contract_addr, cty)
            .map_err(XError::from)?
        else {
            return Err(XError::fault(format!("abst call {:?} not found in {}", cty, adr)));
        };
        // Keep state anchored to the concrete contract address, even when abstract entry body is inherited from a parent owner. This preserves this/self split semantics.
        let rv = self.do_call(
            host,
            exec,
            hit.fnobj.as_ref(),
            FrameBindings::contract(contract_addr, hit.owner, hit.lib_table)
                .with_intent_binding(intent_binding),
            Some(param),
        ).map_err(XError::from)?;
        check_vm_return_value(&rv, &format!("call {}.{:?}", adr, cty))?;
        Ok(rv)
    }

    fn p2sh_call<H: VmHost + ?Sized>(
        &mut self,
        host: &mut H,
        ctype: CodeType,
        p2sh_addr: Address,
        libs: Vec<ContractAddress>,
        codes: ByteView,
        intent_binding: IntentScope,
        param: Value,
    ) -> XRet<Value> {
        // Caller must pre-validate lock script bytes. Production P2SH flow verifies inputs before VM call.
        let fnobj = FnObj::plain(ctype, codes, 0, None);
        let ctx_adr = p2sh_addr;
        let rv = self.do_call(
            host,
            EntryKind::P2sh.root_exec(),
            &fnobj,
            FrameBindings::root(
                ctx_adr,
                libs.into_iter()
                    .map(|addr| addr.into_addr())
                    .collect::<Vec<_>>()
                    .into(),
            )
            .with_intent_binding(intent_binding),
            Some(param),
        ).map_err(XError::from)?;
        check_vm_return_value(&rv, "p2sh call")?;
        Ok(rv)
    }

    fn do_call<H: VmHost + ?Sized>(
        &mut self,
        host: &mut H,
        exec: ExecCtx,
        code: &FnObj,
        bindings: FrameBindings,
        param: Option<Value>,
    ) -> VmrtRes<Value> {
        self.frames.push(CallFrame::new());
        let res = self.frames.last_mut().unwrap().start_call(
            &mut self.r,
            host,
            exec,
            code,
            bindings,
            param,
        );
        self.frames.pop().unwrap().reclaim(&mut self.r);
        res
    }
}
