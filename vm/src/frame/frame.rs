#[derive(Debug, Default)]
pub struct CallFrame {
    frames: Vec<Frame>,
}

impl CallFrame {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn len(&self) -> usize {
        self.frames.len()
    }

    pub fn pop(&mut self) -> Option<Frame> {
        self.frames.pop()
    }

    pub fn push(&mut self, frame: Frame) {
        self.frames.push(frame);
    }

    pub fn increase(&mut self, r: &mut Runtime) -> VmrtRes<Frame> {
        Ok(match self.frames.last() {
            Some(f) => f.next(r),
            None => Frame::new(r),
        })
    }

    pub fn reclaim(mut self, r: &mut Runtime) {
        while let Some(frame) = self.pop() {
            frame.reclaim(r)
        }
    }

    pub fn current_intent_scope(&self) -> IntentScope {
        self.frames
            .last()
            .and_then(|frame| frame.intent_state.current_scope())
    }
}

#[allow(dead_code)]
#[derive(Debug, Clone, Default)]
pub struct IntentScopeState {
    base: IntentScope,
    stack: Vec<BoundIntentId>,
}

impl IntentScopeState {
    pub fn current_scope(&self) -> IntentScope {
        if self.stack.is_empty() {
            self.base
        } else {
            self.stack.last().cloned()
        }
    }

    pub fn current_bound_intent_id(&self) -> BoundIntentId {
        self.current_scope().flatten()
    }

    pub fn base_scope(&self) -> IntentScope {
        self.base
    }

    pub fn len(&self) -> usize {
        self.stack.len()
    }

    pub fn reset(&mut self, base: IntentScope) {
        self.base = base;
        self.stack.clear();
    }

    pub fn push(&mut self, binding: BoundIntentId) {
        self.stack.push(binding);
    }

    pub fn pop(&mut self) -> Option<BoundIntentId> {
        self.stack.pop()
    }
}

#[derive(Debug, Default)]
pub struct Frame {
    pub pc: usize,
    pub exec: ExecCtx,
    pub bindings: FrameBindings,
    pub intent_state: IntentScopeState,
    pub call_argv: Value,
    pub types: Option<FuncArgvTypes>,
    pub codes: ByteView,
    pub oprnds: Stack,
    pub locals: Stack,
    pub heap: Heap,
}

impl Frame {
    pub fn reclaim(self, r: &mut Runtime) {
        r.stack_reclaim(self.oprnds);
        r.stack_reclaim(self.locals);
        r.heap_reclaim(self.heap);
    }

    pub fn new(r: &mut Runtime) -> Self {
        let mut f = Self {
            oprnds: r.stack_allocat(),
            locals: r.stack_allocat(),
            heap: r.heap_allocat(),
            ..Default::default()
        };
        let cap = &r.warm.space_cap;
        f.oprnds.reset(cap.stack_slot);
        f.locals.reset(cap.local_slot);
        f.heap.reset(cap.heap_segment);
        f
    }

    pub fn next(&self, r: &mut Runtime) -> Self {
        let mut f = Self::new(r);
        let stks = self.oprnds.limit() - self.oprnds.len();
        let locs = self.locals.limit() - self.locals.len();
        f.oprnds.reset(stks);
        f.locals.reset(locs);
        f.bindings = self.bindings.clone();
        f
    }

    pub fn pop_value(&mut self) -> VmrtRes<Value> {
        self.oprnds.pop()
    }

    pub fn push_value(&mut self, v: Value) -> VmrtErr {
        self.oprnds.push(v)
    }

    pub fn check_output_type(&self, v: &mut Value, cap: &SpaceCap) -> VmrtErr {
        v.check_func_retv()?;
        v.check_container_cap(cap)?;
        match &self.types {
            Some(ty) => ty.check_output(v),
            None => Ok(()),
        }
    }

    fn clear_runtime_state(&mut self) {
        self.oprnds.clear();
        self.locals.clear();
        self.heap.reset(self.heap.limit());
    }

    fn prepare_common(
        &mut self,
        exec: ExecCtx,
        bindings: FrameBindings,
        fnobj: &FnObj,
        height: u64,
        gas_extra: &GasExtra,
        mut argv: Value,
        have_param: bool,
        cap: &SpaceCap,
    ) -> VmrtErr {
        self.clear_runtime_state();
        if have_param {
            if let Some(vtys) = &fnobj.agvty {
                vtys.check_params(&mut argv)?;
            }
            argv.check_container_cap(cap)?;
            self.oprnds.push(argv.clone())?;
        }
        self.bindings = bindings;
        self.call_argv = argv;
        self.types = fnobj.agvty.clone();
        self.pc = 0;
        self.exec = exec;
        self.codes = fnobj.exec_bytecodes(height, gas_extra)?;
        Ok(())
    }

    pub fn prepare_invoke_unchecked_shape(
        &mut self,
        exec: ExecCtx,
        bindings: FrameBindings,
        fnobj: &FnObj,
        height: u64,
        gas_extra: &GasExtra,
        param: Value,
        cap: &SpaceCap,
    ) -> VmrtErr {
        // Caller must validate argv shape before any contract planning/warmup.
        self.intent_state.reset(bindings.intent_scope);
        self.prepare_common(exec, bindings, fnobj, height, gas_extra, param, true, cap)
    }

    pub fn prepare(
        &mut self,
        exec: ExecCtx,
        bindings: FrameBindings,
        fnobj: &FnObj,
        height: u64,
        gas_extra: &GasExtra,
        param: Option<Value>,
        cap: &SpaceCap,
    ) -> VmrtErr {
        let have_param = param.is_some();
        let argv = param.unwrap_or(Value::Nil);
        if have_param {
            argv.check_func_argv()?;
        }
        self.intent_state.reset(bindings.intent_scope);
        self.prepare_common(exec, bindings, fnobj, height, gas_extra, argv, have_param, cap)
    }

    pub fn prepare_splice(
        &mut self,
        exec: ExecCtx,
        bindings: FrameBindings,
        fnobj: &FnObj,
        height: u64,
        gas_extra: &GasExtra,
        param: Value,
        cap: &SpaceCap,
    ) -> VmrtErr {
        param.check_func_argv()?;
        param.check_container_cap(cap)?;
        let caller_output = match &self.types {
            Some(types) => types
                .output_type()
                .map_err(|e| ItrErr::new(ItrErrCode::CallArgvTypeFail, &e))?,
            None => None,
        };
        let callee_params = match &fnobj.agvty {
            Some(types) => types
                .param_types()
                .map_err(|e| ItrErr::new(ItrErrCode::CallArgvTypeFail, &e))?,
            None => vec![],
        };
        self.types = if caller_output.is_none() && callee_params.is_empty() {
            None
        } else {
            Some(
                FuncArgvTypes::from_types(caller_output, callee_params)
                    .map_err(|e| ItrErr::new(ItrErrCode::CallArgvTypeFail, &e))?,
            )
        };
        self.bindings = bindings;
        self.pc = 0;
        self.exec = exec;
        self.call_argv = param;
        self.codes = fnobj.exec_bytecodes(height, gas_extra)?;
        Ok(())
    }

    pub fn execute<H: VmHost + ?Sized>(
        &mut self,
        r: &mut Runtime,
        host: &mut H,
    ) -> VmrtRes<CallExit> {
        let context_addr = self.bindings.context_addr;
        let current_addr = self
            .bindings
            .code_owner
            .as_ref()
            .map(ContractAddress::to_addr)
            .unwrap_or(context_addr);
        execute_code_in_frame(
            &mut self.pc,
            self.codes.as_slice(),
            self.exec,
            &mut self.oprnds,
            &mut self.locals,
            &mut self.heap,
            &mut self.bindings,
            &mut self.intent_state,
            &context_addr,
            &current_addr,
            &r.warm.gas_table,
            &r.warm.gas_extra,
            &r.warm.space_cap,
            &mut r.warm.gas_use,
            &mut r.volatile.global_map,
            &mut r.volatile.memory_map,
            &mut r.volatile.intents,
            &mut r.volatile.deferred_registry,
            host,
        )
    }
}

#[cfg(test)]
mod frame_boundary_tests {
    use super::*;

    #[test]
    fn check_output_type_rejects_heapslice_without_declared_output_type() {
        let frame = Frame::default();
        let mut retv = Value::HeapSlice((0, 1));
        let err = frame
            .check_output_type(&mut retv, &SpaceCap::new(1))
            .unwrap_err();
        assert!(matches!(err, ItrErr(CastBeFnRetvFail, _)));
    }

    #[test]
    fn check_output_type_rejects_oversize_compo() {
        let frame = Frame::default();
        let mut retv = Value::Compo(
            CompoItem::list(std::collections::VecDeque::from([
                Value::U8(1),
                Value::U8(2),
            ]))
            .unwrap(),
        );
        let mut cap = SpaceCap::new(1);
        cap.compo_length = 1;
        let err = frame.check_output_type(&mut retv, &cap).unwrap_err();
        assert_eq!(err.0, ItrErrCode::OutOfCompoLen);
    }

    #[test]
    fn check_output_type_allows_handle_for_internal_flow() {
        let frame = Frame::default();
        let mut retv = Value::handle(7u32);
        frame
            .check_output_type(&mut retv, &SpaceCap::new(1))
            .unwrap();
    }
}

#[cfg(test)]
mod intent_scope_state_tests {
    use super::*;

    #[test]
    fn intent_scope_state_preserves_tri_state_semantics() {
        let mut state = IntentScopeState::default();
        assert_eq!(state.current_scope(), None);
        assert_eq!(state.current_bound_intent_id(), None);

        state.reset(Some(None));
        assert_eq!(state.current_scope(), Some(None));
        assert_eq!(state.current_bound_intent_id(), None);

        state.push(Some(7));
        assert_eq!(state.current_scope(), Some(Some(7)));
        assert_eq!(state.current_bound_intent_id(), Some(7));
    }
}

#[cfg(test)]
mod splice_prepare_tests {
    use super::*;
    use field::{Address, Uint4};

    fn mk_contract_addr(n: u32) -> ContractAddress {
        let base = Address::from_readable("1MzNY1oA3kfgYi75zquj3SRUPYztzXHzK9").unwrap();
        ContractAddress::calculate(&base, &Uint4::from(n))
    }

    fn mk_bindings(owner: ContractAddress) -> FrameBindings {
        FrameBindings::contract(owner.clone(), owner, Vec::<Address>::new().into())
    }

    #[test]
    fn prepare_splice_preserves_runtime_state_and_signature() {
        let mut res = Runtime::create(1);
        let mut frame = Frame::new(&mut res);
        frame.call_argv = Value::U8(1);
        frame.types =
            Some(FuncArgvTypes::from_types(Some(ValueTy::U8), vec![ValueTy::U8]).unwrap());
        frame.locals.push(Value::U8(9)).unwrap();
        frame.oprnds.push(Value::U8(7)).unwrap();
        let fnobj = FnObj::plain(CodeType::Bytecode, vec![Bytecode::END as u8], 0, None);
        let owner = mk_contract_addr(41);
        let bindings = mk_bindings(owner.clone());
        frame
            .prepare_splice(
                ExecCtx::view(),
                bindings.clone(),
                &fnobj,
                1,
                &res.warm.gas_extra,
                Value::U8(2),
                &res.warm.space_cap,
            )
            .unwrap();
        assert_eq!(frame.bindings.code_owner.as_ref().unwrap(), &owner);
        assert_eq!(frame.exec, ExecCtx::view());
        assert_eq!(frame.pc, 0);
        assert_eq!(frame.locals.len(), 1);
        assert_eq!(frame.oprnds.len(), 1);
        assert_eq!(*frame.oprnds.peek().unwrap(), Value::U8(7));
        assert_eq!(frame.call_argv, Value::U8(2));
        assert_eq!(
            frame.types.as_ref().unwrap().output_type().unwrap(),
            Some(ValueTy::U8)
        );
        assert_eq!(frame.bindings, bindings);
    }
}
