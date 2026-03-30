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

struct VmVolatileSnapshot {
    global_map: GKVMap,
    memory_map: CtcKVMap,
    intents: IntentRuntime,
    deferred_registry: DeferredRegistry,
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

    pub fn sandbox_main_call_raw(
        &mut self,
        ctx: &mut dyn Context,
        ctype: CodeType,
        codes: Arc<[u8]>,
    ) -> Ret<Value> {
        let (_, ret_val) = self.sandbox_main_call_raw_with_gas(ctx, ctype, codes)?;
        Ok(ret_val)
    }

    pub fn main_call(
        &mut self,
        ctx: &mut dyn Context,
        ctype: CodeType,
        codes: Arc<[u8]>,
    ) -> XRet<Value> {
        let req = VmEntryReq::Main { code_type: ctype, codes };
        let (_, retv) = self.execute_entry_req(ctx, req)?;
        Ok(retv)
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
        self.run_deferred_entries(ctx)
    }

    fn call(&mut self, ctx: &mut dyn Context, req: Box<dyn Any>) -> XRet<(VmGasBuckets, Box<dyn Any>)> {
        self.dispatch_entry_call(ctx, req)
    }
}

/*********************************/

#[allow(dead_code)]
pub struct Machine {
    frames: Vec<Box<CallFrame>>,
}

impl Machine {
    pub fn create(r: Resoure) -> MachineBox {
        MachineBox::from_resource(r)
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
        Self::validate_vm_entry_param(runtime, &EntryKind::Main.root_exec(), None)?;
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
        Self::validate_vm_entry_param(runtime, &exec, Some(&param))?;
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
        Self::validate_vm_entry_param(runtime, &exec, Some(&param))?;
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
