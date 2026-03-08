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

    pub fn increase(&mut self, r: &mut Resoure) -> VmrtRes<Frame> {
        Ok(match self.frames.last() {
            Some(f) => f.next(r),
            None => Frame::new(r),
        })
    }

    pub fn reclaim(mut self, r: &mut Resoure) {
        while let Some(frame) = self.pop() {
            frame.reclaim(r)
        }
    }
}

#[allow(dead_code)]
#[derive(Debug, Default)]
pub struct Frame {
    pub pc: usize,
    pub exec: ExecCtx,
    pub bindings: FrameBindings,
    pub call_argv: Value,
    pub types: Option<FuncArgvTypes>,
    pub ret_check: RetCheck,
    pub codes: ByteView,
    pub oprnds: Stack,
    pub locals: Stack,
    pub heap: Heap,
}

#[derive(Debug, Clone, Default)]
pub enum RetCheck {
    #[default]
    Callee,
    Caller(Option<FuncArgvTypes>),
}

impl RetCheck {
    pub fn bind_for_reuse(&self, current_types: &Option<FuncArgvTypes>, bind: ReturnBind) -> Self {
        match bind {
            ReturnBind::Callee => Self::Callee,
            ReturnBind::Caller => match self {
                Self::Callee => Self::Caller(current_types.clone()),
                Self::Caller(types) => Self::Caller(types.clone()),
            },
        }
    }

    pub fn check(&self, current_types: &Option<FuncArgvTypes>, v: &mut Value) -> VmrtErr {
        v.canbe_func_retv()?;
        match self {
            Self::Callee => match current_types {
                Some(ty) => ty.check_output(v),
                None => Ok(()),
            },
            Self::Caller(Some(caller_types)) => caller_types.check_output(v),
            Self::Caller(None) => Ok(()),
        }
    }
}

impl Frame {
    pub fn reclaim(self, r: &mut Resoure) {
        r.stack_reclaim(self.oprnds);
        r.stack_reclaim(self.locals);
        r.heap_reclaim(self.heap);
    }

    pub fn new(r: &mut Resoure) -> Self {
        let mut f = Self {
            oprnds: r.stack_allocat(),
            locals: r.stack_allocat(),
            heap: r.heap_allocat(),
            ..Default::default()
        };
        let cap = &r.space_cap;
        f.oprnds.reset(cap.stack_slot);
        f.locals.reset(cap.local_slot);
        f.heap.reset(cap.heap_segment);
        f
    }

    pub fn next(&self, r: &mut Resoure) -> Self {
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

    pub fn check_output_type(&self, v: &mut Value) -> VmrtErr {
        RetCheck::Callee.check(&self.types, v)
    }

    pub fn check_return_value(&self, v: &mut Value) -> VmrtErr {
        self.ret_check.check(&self.types, v)
    }

    pub fn bind_reuse_return(&self, bind: ReturnBind) -> RetCheck {
        self.ret_check.bind_for_reuse(&self.types, bind)
    }

    fn clear_runtime_state(&mut self) {
        self.oprnds.clear();
        self.locals.clear();
        self.heap.reset(self.heap.limit());
    }

    pub fn prepare(
        &mut self,
        exec: ExecCtx,
        bindings: FrameBindings,
        fnobj: &FnObj,
        height: u64,
        param: Option<Value>,
    ) -> VmrtErr {
        self.clear_runtime_state();
        self.ret_check = RetCheck::Callee;
        let have_param = param.is_some();
        let mut argv = param.unwrap_or(Value::Nil);
        if have_param {
            argv.canbe_func_argv()?;
            if let Some(vtys) = &fnobj.agvty {
                vtys.check_params(&mut argv)?;
            }
            self.oprnds.push(argv.clone())?;
        }
        self.bindings = bindings;
        self.call_argv = argv;
        self.types = fnobj.agvty.clone();
        self.pc = 0;
        self.exec = exec;
        self.codes = fnobj.exec_bytecodes(height)?;
        Ok(())
    }

    pub fn prepare_reuse_call(
        &mut self,
        exec: ExecCtx,
        bindings: FrameBindings,
        fnobj: &FnObj,
        height: u64,
        ret_check: RetCheck,
    ) -> VmrtErr {
        let mut argv = self.call_argv.clone();
        argv.canbe_func_argv()?;
        if let Some(vtys) = &fnobj.agvty {
            vtys.check_params(&mut argv)?;
        }
        self.clear_runtime_state();
        self.bindings = bindings;
        self.call_argv = argv.clone();
        self.oprnds.push(argv)?;
        self.types = fnobj.agvty.clone();
        self.pc = 0;
        self.exec = exec;
        self.codes = fnobj.exec_bytecodes(height)?;
        self.ret_check = ret_check;
        Ok(())
    }

    pub fn execute(&mut self, r: &mut Resoure, env: &mut ExecEnv) -> VmrtRes<CallExit> {
        let mut host = crate::machine::CtxHost::new(env.ctx);
        execute_code(
            &mut self.pc,
            self.codes.as_slice(),
            self.exec,
            &mut self.oprnds,
            &mut self.locals,
            &mut self.heap,
            &self.bindings.context_addr,
            self.bindings.current_addr(),
            env.gas,
            &r.gas_table,
            &r.gas_extra,
            &r.space_cap,
            &mut r.global_map,
            &mut r.memory_map,
            &mut host,
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
        let err = frame.check_output_type(&mut retv).unwrap_err();
        assert!(matches!(err, ItrErr(CastBeFnRetvFail, _)));
    }
}

#[cfg(test)]
mod tail_reuse_prepare_tests {
    use super::*;
    use field::{Address, Uint4};

    fn mk_contract_addr(n: u32) -> ContractAddress {
        let base = Address::from_readable("1MzNY1oA3kfgYi75zquj3SRUPYztzXHzK9").unwrap();
        ContractAddress::calculate(&base, &Uint4::from(n))
    }

    fn mk_bindings(owner: ContractAddress) -> FrameBindings {
        FrameBindings::contract(
            owner.clone(),
            owner.clone(),
            owner,
            Vec::<ContractAddress>::new().into(),
        )
    }

    #[test]
    fn prepare_tail_reuse_rebinds_types_and_ret_policy() {
        let mut res = Resoure::create(1);
        let mut frame = Frame::new(&mut res);
        frame.call_argv = Value::U8(1);
        frame.types =
            Some(FuncArgvTypes::from_types(Some(ValueTy::U8), vec![ValueTy::U8]).unwrap());
        frame.ret_check = RetCheck::Caller(None);
        frame.locals.push(Value::U8(9)).unwrap();
        let callee_types =
            FuncArgvTypes::from_types(Some(ValueTy::Bool), vec![ValueTy::U8]).unwrap();
        let fnobj = FnObj::plain(
            CodeType::Bytecode,
            vec![Bytecode::END as u8],
            0,
            Some(callee_types),
        );
        let owner = mk_contract_addr(41);
        let bindings = mk_bindings(owner.clone());
        frame
            .prepare_reuse_call(
                ExecCtx::view(),
                bindings.clone(),
                &fnobj,
                1,
                RetCheck::Callee,
            )
            .unwrap();
        assert!(matches!(frame.ret_check, RetCheck::Callee));
        assert_eq!(
            frame.types.as_ref().unwrap().output_type().unwrap(),
            Some(ValueTy::Bool)
        );
        let mut retv = Value::Bool(true);
        frame.check_output_type(&mut retv).unwrap();
        assert_eq!(frame.bindings.code_owner.as_ref().unwrap(), &owner);
        assert_eq!(frame.exec, ExecCtx::view());
        assert_eq!(frame.pc, 0);
        assert_eq!(frame.locals.len(), 0);
        assert_eq!(frame.oprnds.len(), 1);
        assert_eq!(*frame.oprnds.peek().unwrap(), Value::U8(1));
        assert_eq!(frame.bindings, bindings);
    }

    #[test]
    fn prepare_tail_reuse_clears_previous_signature_when_callee_has_none() {
        let mut res = Resoure::create(1);
        let mut frame = Frame::new(&mut res);
        frame.call_argv = Value::Nil;
        frame.types = Some(FuncArgvTypes::from_types(Some(ValueTy::U8), vec![]).unwrap());
        frame.ret_check = RetCheck::Caller(None);
        let fnobj = FnObj::plain(CodeType::Bytecode, vec![Bytecode::END as u8], 0, None);
        frame
            .prepare_reuse_call(
                ExecCtx::main(),
                mk_bindings(mk_contract_addr(42)),
                &fnobj,
                1,
                RetCheck::Callee,
            )
            .unwrap();
        assert!(matches!(frame.ret_check, RetCheck::Callee));
        assert!(frame.types.is_none());
        let mut retv = Value::Bytes(vec![]);
        frame.check_output_type(&mut retv).unwrap();
    }

    #[test]
    fn bind_reuse_return_preserves_outer_caller_contract() {
        let mut res = Resoure::create(1);
        let mut frame = Frame::new(&mut res);
        let outer_types = FuncArgvTypes::from_types(Some(ValueTy::U8), vec![]).unwrap();
        let middle_types = FuncArgvTypes::from_types(Some(ValueTy::Bool), vec![]).unwrap();
        frame.types = Some(middle_types);
        frame.ret_check = RetCheck::Caller(Some(outer_types.clone()));
        let rebound = frame.bind_reuse_return(ReturnBind::Caller);
        match rebound {
            RetCheck::Caller(Some(types)) => {
                assert_eq!(types.output_type().unwrap(), outer_types.output_type().unwrap());
            }
            other => panic!("unexpected ret_check: {other:?}"),
        }
    }
}
