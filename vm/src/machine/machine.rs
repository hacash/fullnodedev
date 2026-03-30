pub struct MachineRuntime {
    r: Resoure,
}

impl MachineRuntime {
    pub fn new(r: Resoure) -> Self {
        Self { r }
    }
}

impl std::ops::Deref for MachineRuntime {
    type Target = Resoure;

    fn deref(&self) -> &Self::Target {
        &self.r
    }
}

impl std::ops::DerefMut for MachineRuntime {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.r
    }
}

pub(crate) enum VmEntryReq {
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

impl VmEntryReq {
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

    fn execute(
        self,
        machine: &mut Machine,
        runtime: &mut MachineRuntime,
        ctx: &mut dyn Context,
    ) -> XRet<Value> {
        match self {
            Self::Main { code_type, codes } => machine.main_call(runtime, ctx, code_type, codes),
            Self::P2sh {
                code_type,
                state_addr,
                libs,
                codes,
                intent_binding,
                param,
            } => machine.p2sh_call(runtime, ctx, code_type, state_addr, libs, codes, intent_binding, param),
            Self::Abst {
                kind,
                contract_addr,
                intent_binding,
                param,
            } => machine.abst_call(runtime, ctx, kind, contract_addr, intent_binding, param),
        }
    }
}

/*********************************/

struct VmEntryFrame {
    kind: EntryKind,
    gas_base: GasUse,
    min_cost: i64,
}

struct VmVolatileSnapshot {
    global_map: GKVMap,
    memory_map: CtcKVMap,
    intents: IntentRuntime,
    deferred_registry: DeferredRegistry,
}

#[derive(Clone, Copy)]
enum VmEntryMode {
    KeepCurrentExecFrom,
    ForceCall,
    AssumeAlreadyCall,
}

enum VmEntryFailure {
    Message(String),
    Runtime(crate::rt::ItrErr),
}

struct VmEntryGuard {
    entries: *mut Vec<VmEntryFrame>,
    entry: VmEntryFrame,
    index: usize,
    armed: bool,
}

impl VmEntryGuard {
    fn push<E>(
        entries: &mut Vec<VmEntryFrame>,
        max_reentry: u32,
        gas_base: GasUse,
        kind: EntryKind,
        min_cost: i64,
        map: fn(VmEntryFailure) -> E,
    ) -> Result<Self, E> {
        let next_level = entries
            .len()
            .checked_add(1)
            .ok_or_else(|| map(VmEntryFailure::Message("re-entry level overflow".to_owned())))?;
        if next_level as u32 > max_reentry + 1 {
            return Err(map(VmEntryFailure::Message(format!(
                "re-entry level {} exceeded limit {}",
                next_level - 1,
                max_reentry
            ))));
        }
        let entry = VmEntryFrame {
            kind,
            gas_base,
            min_cost,
        };
        let index = entries.len();
        entries.push(VmEntryFrame {
            kind: entry.kind,
            gas_base: entry.gas_base,
            min_cost: entry.min_cost,
        });
        Ok(Self {
            entries,
            entry,
            index,
            armed: true,
        })
    }

    fn entry(&self) -> VmEntryFrame {
        VmEntryFrame {
            kind: self.entry.kind,
            gas_base: self.entry.gas_base,
            min_cost: self.entry.min_cost,
        }
    }

    fn disarm(mut self) -> VmEntryFrame {
        self.armed = false;
        self.entry()
    }
}

impl Drop for VmEntryGuard {
    fn drop(&mut self) {
        if !self.armed {
            return;
        }
        let entries = unsafe { &mut *self.entries };
        let Some(popped) = entries.pop() else {
            debug_assert!(false, "vm entry frame missing during guard drop");
            return;
        };
        debug_assert_eq!(entries.len(), self.index);
        debug_assert_eq!(popped.kind as u8, self.entry.kind as u8);
    }
}

#[allow(dead_code)]
pub struct MachineBox {
    runtime: Option<MachineRuntime>,
    machine: Machine,
    entries: Vec<VmEntryFrame>,
}

impl Drop for MachineBox {
    fn drop(&mut self) {
        if let Some(runtime) = self.runtime.take() {
            global_machine_manager().reclaim(runtime.r);
        }
    }
}

impl MachineBox {
    pub fn from_resource(r: Resoure) -> Self {
        Self {
            runtime: Some(MachineRuntime::new(r)),
            machine: Machine::new(),
            entries: vec![],
        }
    }

    #[inline]
    fn runtime_ref(&self) -> Ret<&MachineRuntime> {
        self.runtime
            .as_ref()
            .ok_or_else(|| "machine runtime missing".to_owned())
    }

    #[inline]
    fn runtime_mut(&mut self) -> Ret<&mut MachineRuntime> {
        self.runtime
            .as_mut()
            .ok_or_else(|| "machine runtime missing".to_owned())
    }

    fn map_ret_entry_failure(err: VmEntryFailure) -> String {
        match err {
            VmEntryFailure::Message(msg) => msg,
            VmEntryFailure::Runtime(err) => err.to_string(),
        }
    }

    fn map_xret_entry_failure(err: VmEntryFailure) -> XError {
        match err {
            VmEntryFailure::Message(msg) => XError::fault(msg),
            VmEntryFailure::Runtime(err) => XError::from(err),
        }
    }

    fn append_secondary_message(primary: String, secondary: String) -> String {
        if secondary.is_empty() || primary.contains(&secondary) {
            primary
        } else {
            format!("{} | secondary: {}", primary, secondary)
        }
    }

    fn merge_ret_entry_errors(exec_err: String, settle_err: String) -> String {
        Self::append_secondary_message(exec_err, settle_err)
    }

    fn merge_xret_entry_errors(exec_err: XError, settle_err: XError) -> XError {
        let exec_is_revert = exec_err.is_revert();
        let settle_is_revert = settle_err.is_revert();
        if exec_is_revert && !settle_is_revert {
            match settle_err {
                sys::ExecError::Revert(msg) => XError::fault(Self::append_secondary_message(msg, exec_err.to_string())),
                sys::ExecError::Fault(msg) => XError::fault(Self::append_secondary_message(msg, exec_err.to_string())),
            }
        } else {
            match exec_err {
                sys::ExecError::Revert(msg) => XError::revert(Self::append_secondary_message(msg, settle_err.to_string())),
                sys::ExecError::Fault(msg) => XError::fault(Self::append_secondary_message(msg, settle_err.to_string())),
            }
        }
    }

    fn with_entry_mode<T>(
        ctx: &mut dyn Context,
        mode: VmEntryMode,
        f: impl for<'a> FnOnce(&'a mut dyn Context) -> T,
    ) -> T {
        match mode {
            VmEntryMode::KeepCurrentExecFrom => f(ctx),
            VmEntryMode::ForceCall => basis::interface::with_exec_from(
                ctx,
                basis::component::ExecFrom::Call,
                f,
            ),
            VmEntryMode::AssumeAlreadyCall => f(ctx),
        }
    }

    fn run_vm_entry<T, E>(
        &mut self,
        ctx: &mut dyn Context,
        kind: EntryKind,
        min_cost: i64,
        mode: VmEntryMode,
        execute: impl FnOnce(&mut Machine, &mut MachineRuntime, &mut dyn Context) -> Result<T, E>,
        map: fn(VmEntryFailure) -> E,
        merge: fn(E, E) -> E,
    ) -> Result<(GasUse, T), E> {
        let (max_reentry, gas_base) = {
            let runtime = self
                .runtime_ref()
                .map_err(|e| map(VmEntryFailure::Message(e)))?;
            (runtime.warm.space_cap.reentry_level, runtime.gas_use())
        };
        let guard = VmEntryGuard::push(&mut self.entries, max_reentry, gas_base, kind, min_cost, map)?;
        let result = Self::with_entry_mode(ctx, mode, |ctx| {
            let machine = &mut self.machine;
            let Some(runtime) = self.runtime.as_mut() else {
                return Err(map(VmEntryFailure::Message("machine runtime missing".to_owned())));
            };
            execute(machine, runtime, ctx)
        });
        let entry = guard.disarm();
        let settle = match self.runtime_mut() {
            Ok(runtime) => {
                let mut cost = runtime
                    .gas_use()
                    .checked_sub(entry.gas_base)
                    .ok_or_else(|| {
                        map(VmEntryFailure::Message(format!(
                            "gas cost underflow: total={:?}, base={:?}",
                            runtime.gas_use(),
                            entry.gas_base
                        )))
                    })?;
                if cost.total() < entry.min_cost {
                    let delta = entry.min_cost - cost.total();
                    runtime
                        .settle_compute_gas(ctx, delta)
                        .map_err(|e| map(VmEntryFailure::Runtime(e)))?;
                    cost.compute += delta;
                }
                if cost.total() <= 0 {
                    Err(map(VmEntryFailure::Message(format!(
                        "{:?} gas cost invalid: {}",
                        entry.kind,
                        cost.total()
                    ))))
                } else {
                    Ok(cost)
                }
            }
            Err(e) => Err(map(VmEntryFailure::Message(e))),
        };
        match (result, settle) {
            (Err(exec_err), Err(settle_err)) => Err(merge(exec_err, settle_err)),
            (Err(exec_err), _) => Err(exec_err),
            (Ok(_), Err(settle_err)) => Err(settle_err),
            (Ok(retv), Ok(cost)) => Ok((cost, retv)),
        }
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
        let min_cost = {
            let runtime = self.runtime_ref().map_err(|e| e.to_string())?;
            EntryKind::Main.min_call_cost(&runtime.warm.gas_extra)
        };
        self.run_vm_entry(
            ctx,
            EntryKind::Main,
            min_cost,
            VmEntryMode::KeepCurrentExecFrom,
            move |machine, runtime, ctx| machine.main_call_raw(runtime, ctx, ctype, codes),
            Self::map_ret_entry_failure,
            Self::merge_ret_entry_errors,
        )
    }

    fn execute_entry_req(
        &mut self,
        ctx: &mut dyn Context,
        req: VmEntryReq,
    ) -> XRet<(GasUse, Value)> {
        let (kind, min_cost) = {
            let runtime = self.runtime_ref().map_err(XError::fault)?;
            (req.entry_kind(), req.min_call_cost(&runtime.warm.gas_extra))
        };
        self.run_vm_entry(
            ctx,
            kind,
            min_cost,
            VmEntryMode::AssumeAlreadyCall,
            move |machine, runtime, ctx| req.execute(machine, runtime, ctx),
            Self::map_xret_entry_failure,
            Self::merge_xret_entry_errors,
        )
    }
}

impl VM for MachineBox {
    fn current_intent_scope(&mut self) -> Option<Option<usize>> {
        self.machine.current_intent_scope()
    }

    fn runtime_config(&mut self) -> Option<Box<dyn Any>> {
        self.runtime_ref()
            .ok()
            .map(|r| Box::new((r.warm.gas_extra.clone(), r.warm.space_cap.clone())) as Box<dyn Any>)
    }

    fn snapshot_volatile(&mut self) -> Box<dyn Any> {
        match self.runtime_ref() {
            Ok(r) => Box::new(VmVolatileSnapshot {
                global_map: r.volatile.global_map.clone(),
                memory_map: r.volatile.memory_map.clone(),
                intents: r.volatile.intents.clone(),
                deferred_registry: r.volatile.deferred_registry.clone(),
            }),
            Err(e) => {
                debug_assert!(false, "snapshot_volatile: {}", e);
                Box::new(VmVolatileSnapshot {
                    global_map: GKVMap::default(),
                    memory_map: CtcKVMap::default(),
                    intents: IntentRuntime::default(),
                    deferred_registry: DeferredRegistry::default(),
                })
            }
        }
    }

    fn restore_volatile(&mut self, snap: Box<dyn Any>) {
        let Ok(snap) = snap.downcast::<VmVolatileSnapshot>() else {
            debug_assert!(false, "restore_volatile: snapshot type mismatch");
            return;
        };
        let snap = *snap;
        if let Ok(r) = self.runtime_mut() {
            r.volatile.global_map = snap.global_map;
            r.volatile.memory_map = snap.memory_map;
            r.volatile.intents = snap.intents;
            r.volatile.deferred_registry = snap.deferred_registry;
        } else {
            debug_assert!(false, "restore_volatile: machine missing");
        }
    }

    fn rollback_volatile_preserve_warm_and_gas(&mut self) {
        if let Ok(r) = self.runtime_mut() {
            r.volatile.global_map.clear();
            r.volatile.memory_map.clear();
            r.volatile.intents.clear();
            r.volatile.deferred_registry.clear();
        }
    }

    fn invalidate_contract_cache(&mut self, addr: &Address) {
        let Ok(caddr) = ContractAddress::from_addr(*addr) else {
            return;
        };
        if let Ok(r) = self.runtime_mut() {
            r.warm.contracts.remove(&caddr);
        }
        global_machine_manager()
            .contract_cache()
            .remove_addr(&caddr);
    }

    fn drain_deferred(&mut self, ctx: &mut dyn Context) -> Rerr {
        let callbacks = {
            let r = self.runtime_mut().map_err(|e| e.to_string())?;
            // Deferred phase currently uses strict one-shot consumption: once drained, callbacks are consumed
            // even if a later deferred callback fails. This keeps deferred dispatch non-reentrant and matches the
            // existing transaction semantics where warmups/gas remain monotonic after the phase begins.
            r.volatile.deferred_registry.drain_lifo()
        };
        for caddr in callbacks {
            let _ = self
                .run_vm_entry(
                    ctx,
                    EntryKind::Abst,
                    {
                        let runtime = self.runtime_ref().map_err(|e| e.to_string())?;
                        EntryKind::Abst.min_call_cost(&runtime.warm.gas_extra)
                    },
                    VmEntryMode::ForceCall,
                    move |machine, runtime, ctx| {
                        VmEntryReq::Abst {
                            kind: AbstCall::Deferred,
                            contract_addr: caddr.addr,
                            intent_binding: Some(caddr.intent_id),
                            param: Value::Nil,
                        }
                        .execute(machine, runtime, ctx)
                        .map_err(|e| e.to_string())
                    },
                    Self::map_ret_entry_failure,
                    Self::merge_ret_entry_errors,
                )
                .map_err(|e| e.to_string())?;
        }
        Ok(())
    }

    fn call(&mut self, ctx: &mut dyn Context, req: Box<dyn Any>) -> XRet<(GasUse, Box<dyn Any>)> {
        let Ok(req) = req.downcast::<VmEntryReq>() else {
            return xerrf!("vm call request type mismatch");
        };
        let (cost, resv) = self.execute_entry_req(ctx, *req)?;
        Ok((cost, Box::new(resv)))
    }
}

/*********************************/

#[allow(dead_code)]
pub struct Machine {
    frames: Vec<Box<CallFrame>>,
}

impl Machine {
    fn validate_vm_entry_param(
        kind: EntryKind,
        runtime: &MachineRuntime,
        exec: &ExecCtx,
        param: Option<&Value>,
    ) -> XRerr {
        exec.ensure_call_depth(&runtime.warm.space_cap).map_err(XError::from)?;
        if let Some(param) = param {
            param.check_vm_boundary_argv().map_err(XError::from)?;
            param
                .check_container_cap(&runtime.warm.space_cap)
                .map_err(XError::from)?;
        }
        let _ = kind;
        Ok(())
    }

    fn validate_vm_entry_return(kind: EntryKind, rv: &Value, err_msg: &str) -> XRerr {
        match kind {
            EntryKind::Main | EntryKind::P2sh | EntryKind::Abst => check_vm_return_value(rv, err_msg),
        }
    }

    fn current_intent_scope(&self) -> IntentScope {
        self.frames.last().and_then(|frame| frame.current_intent_scope())
    }

    pub fn is_in_calling(&self) -> bool {
        !self.frames.is_empty()
    }

    pub fn new() -> Self {
        Self { frames: vec![] }
    }

    pub fn main_call_raw<H: VmHost + ?Sized>(
        &mut self,
        runtime: &mut MachineRuntime,
        host: &mut H,
        ctype: CodeType,
        codes: Arc<[u8]>,
    ) -> Ret<Value> {
        // Caller must pre-validate code bytes. Production entry actions run convert_and_check before setup_vm_run.
        let fnobj = FnObj::plain(ctype, codes, 0, None);
        let rv = self.do_call(
            runtime,
            host,
            EntryKind::Main.root_exec(),
            &fnobj,
            host.main_entry_bindings(),
            None,
        )?;
        rv.check_vm_boundary_retv()?;
        Ok(rv)
    }

    pub fn main_call<H: VmHost + ?Sized>(
        &mut self,
        runtime: &mut MachineRuntime,
        host: &mut H,
        ctype: CodeType,
        codes: Arc<[u8]>,
    ) -> XRet<Value> {
        Self::validate_vm_entry_param(EntryKind::Main, runtime, &EntryKind::Main.root_exec(), None)?;
        let rv = self.main_call_raw(runtime, host, ctype, codes)?;
        Self::validate_vm_entry_return(EntryKind::Main, &rv, "main call")?;
        Ok(rv)
    }

    pub fn abst_call<H: VmHost + ?Sized>(
        &mut self,
        runtime: &mut MachineRuntime,
        host: &mut H,
        cty: AbstCall,
        contract_addr: ContractAddress,
        intent_binding: IntentScope,
        param: Value,
    ) -> XRet<Value> {
        let exec = EntryKind::Abst.root_exec();
        Self::validate_vm_entry_param(EntryKind::Abst, runtime, &exec, Some(&param))?;
        let adr = contract_addr.to_readable();
        let Some(hit) = runtime
            .resolve_abstfn(host, &contract_addr, cty)
            .map_err(XError::from)?
        else {
            return Err(XError::fault(format!("abst call {:?} not found in {}", cty, adr)));
        };
        // Keep state anchored to the concrete contract address, even when abstract entry body is inherited from a parent owner. This preserves this/self split semantics.
        let rv = self.do_call(
            runtime,
            host,
            exec,
            hit.fnobj.as_ref(),
            FrameBindings::contract(contract_addr, hit.owner, hit.lib_table)
                .with_intent_binding(intent_binding),
            Some(param),
        ).map_err(XError::from)?;
        Self::validate_vm_entry_return(EntryKind::Abst, &rv, &format!("call {}.{:?}", adr, cty))?;
        Ok(rv)
    }

    fn p2sh_call<H: VmHost + ?Sized>(
        &mut self,
        runtime: &mut MachineRuntime,
        host: &mut H,
        ctype: CodeType,
        p2sh_addr: Address,
        libs: Vec<ContractAddress>,
        codes: ByteView,
        intent_binding: IntentScope,
        param: Value,
    ) -> XRet<Value> {
        // Caller must pre-validate lock script bytes. Production P2SH flow verifies inputs before VM call.
        let exec = EntryKind::P2sh.root_exec();
        Self::validate_vm_entry_param(EntryKind::P2sh, runtime, &exec, Some(&param))?;
        let fnobj = FnObj::plain(ctype, codes, 0, None);
        let ctx_adr = p2sh_addr;
        let rv = self.do_call(
            runtime,
            host,
            exec,
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
        Self::validate_vm_entry_return(EntryKind::P2sh, &rv, "p2sh call")?;
        Ok(rv)
    }

    fn do_call<H: VmHost + ?Sized>(
        &mut self,
        runtime: &mut MachineRuntime,
        host: &mut H,
        exec: ExecCtx,
        code: &FnObj,
        bindings: FrameBindings,
        param: Option<Value>,
    ) -> VmrtRes<Value> {
        self.frames.push(Box::new(CallFrame::new()));
        // Keep the active root call frame at a stable heap address so re-entry can grow `self.frames` safely.
        let root_ptr = self
            .frames
            .last_mut()
            .map(|frame| frame.as_mut() as *mut CallFrame)
            .unwrap();
        let res = unsafe {
            (*root_ptr).start_call(
                runtime,
                host,
                exec,
                code,
                bindings,
                param,
            )
        };
        let root = *self.frames.pop().unwrap();
        root.reclaim(runtime);
        res
    }
}
