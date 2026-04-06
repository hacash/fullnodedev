struct VmVolatileSnapshot {
    global_map: GKVMap,
    memory_map: CtcKVMap,
    intents: IntentRuntime,
    deferred_registry: DeferredRegistry,
}

#[allow(dead_code)]
pub struct Executor {
    runtime: std::mem::ManuallyDrop<Runtime>,
    machine: Machine,
    entries: Vec<VmEntryFrame>,
}

impl Drop for Executor {
    fn drop(&mut self) {
        // SAFETY: `runtime` is wrapped in `ManuallyDrop` specifically so drop can move it back into the pool exactly once.
        // After `take`, `self.runtime` must not be read again.
        let runtime = unsafe { std::mem::ManuallyDrop::take(&mut self.runtime) };
        global_runtime_pool().checkin(runtime);
    }
}

impl Executor {
    pub fn from_runtime(r: Runtime) -> Self {
        Self {
            runtime: std::mem::ManuallyDrop::new(r),
            machine: Machine::new(),
            entries: vec![],
        }
    }
}

impl Executor {
    fn runtime_config_any(&mut self) -> Option<Box<dyn Any>> {
        let r = &self.runtime;
        Some(Box::new((r.warm.gas_extra.clone(), r.warm.space_cap.clone())) as Box<dyn Any>)
    }

    fn snapshot_volatile_state(&mut self) -> Box<dyn Any> {
        let r = &self.runtime;
        Box::new(VmVolatileSnapshot {
            global_map: r.volatile.global_map.clone(),
            memory_map: r.volatile.memory_map.clone(),
            intents: r.volatile.intents.clone(),
            deferred_registry: r.volatile.deferred_registry.clone(),
        })
    }

    fn restore_volatile_state(&mut self, snap: Box<dyn Any>) {
        let Ok(snap) = snap.downcast::<VmVolatileSnapshot>() else {
            debug_assert!(false, "restore_volatile: snapshot type mismatch");
            return;
        };
        let snap = *snap;
        let r = &mut self.runtime;
        r.volatile.global_map = snap.global_map;
        r.volatile.memory_map = snap.memory_map;
        r.volatile.intents = snap.intents;
        r.volatile.deferred_registry = snap.deferred_registry;
    }

    fn rollback_volatile_state_preserve_warm_and_gas(&mut self) {
        let r = &mut self.runtime;
        r.volatile.global_map.clear();
        r.volatile.memory_map.clear();
        r.volatile.intents.clear();
        r.volatile.deferred_registry.clear();
    }

    fn invalidate_runtime_contract_cache(&mut self, addr: &Address) {
        let Ok(caddr) = ContractAddress::from_addr(*addr) else {
            return;
        };
        self.runtime.warm.contracts.remove(&caddr);
        global_runtime_pool()
            .contract_cache()
            .remove_addr(&caddr);
    }
}

/*********************************/

#[allow(dead_code)]
struct Machine {
    frames: Vec<Box<CallFrame>>,
}

impl Machine {

    fn current_intent_scope(&self) -> IntentScope {
        self.frames.last().and_then(|frame| frame.current_intent_scope())
    }

    pub fn new() -> Self {
        Self { frames: vec![] }
    }

    pub fn main_call_raw<H: VmHost + ?Sized>(
        &mut self,
        runtime: &mut Runtime,
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

    fn do_call<H: VmHost + ?Sized>(
        &mut self,
        runtime: &mut Runtime,
        host: &mut H,
        exec: ExecCtx,
        code: &FnObj,
        bindings: FrameBindings,
        param: Option<Value>,
    ) -> VmrtRes<Value> {
        self.frames.push(Box::new(CallFrame::new()));
        // Keep the active root call frame at a stable heap address so re-entry can grow `self.frames` safely.
        // SAFETY: the frame itself lives on the heap inside `Box<CallFrame>`, so Vec growth may move the box pointer
        // in the Vec but does not move the pointee. The pointee remains valid until we pop and reclaim it below.
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
