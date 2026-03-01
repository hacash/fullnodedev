// println!("CALLCODE() state_addr={}, code_owner={}", state_addr.prefix(7), code_owner.prefix(7));

impl CallFrame {
    fn prepare_frame(
        frame: &mut Frame,
        mode: ExecMode,
        in_callcode: bool,
        fnobj: &FnObj,
        height: u64,
        param: Option<Value>,
    ) -> VmrtErr {
        frame.prepare(mode, in_callcode, fnobj, height, param)
    }

    pub fn start_call(
        &mut self,
        r: &mut Resoure,
        env: &mut ExecEnv,
        mode: ExecMode,
        code: &FnObj,
        entry_addr: ContractAddress,
        code_owner: Option<ContractAddress>,
        libs: Option<Vec<ContractAddress>>,
        param: Option<Value>,
    ) -> VmrtRes<Value> {
        // Macro definitions for frame access
        macro_rules! curr {
            () => {
                self.frames.last_mut().unwrap()
            };
        }
        macro_rules! curr_ref {
            () => {
                self.frames.last().unwrap()
            };
        }

        use CallExit::*;
        use ExecMode::*;

        let libs_none: Option<Vec<ContractAddress>> = None;
        let height = env.ctx.env().block.height;

        // Setup root frame (depth=0, nested frames get depth+1 via Frame::next)
        let mut root = self.increase(r)?;
        root.state_addr = entry_addr.clone();
        root.code_owner = code_owner.unwrap_or(entry_addr);
        self.push(root);
        Self::prepare_frame(curr!(), mode, false, code, height, param)?;

        // Main execution loop
        loop {
            let exit = curr!().execute(r, env)?;

            match exit {
                Call(fnptr) => {
                    // Load call context
                    let (state_addr, code_owner, depth) = (
                        curr_ref!().state_addr.clone(),
                        curr_ref!().code_owner.clone(),
                        curr_ref!().depth,
                    );

                    let libs_ptr = if depth == 0 { &libs } else { &libs_none };
                    let plan = r.plan_call(
                        env.ctx,
                        &mut *env.gas,
                        CallPlanReq {
                            fptr: fnptr.clone(),
                            state_addr: &state_addr,
                            code_owner: &code_owner,
                            tx_libs: libs_ptr,
                        },
                    )?;
                    let fnobj_arc = plan.fnobj().clone();
                    let fnobj = fnobj_arc.as_ref();
                    let fn_is_public = fnobj.check_conf(FnConf::Public);

                    // CALLCODE: in-place execution
                    if fnptr.is_callcode {
                        curr!().code_owner = plan.code_owner().clone();
                        let callcode_param_count = match &fnobj.agvty {
                            Some(types) => types.param_count(),
                            None => 0,
                        };
                        if callcode_param_count != 0 {
                            return itr_err_fmt!(
                                CallArgvTypeFail,
                                "callcode target must have 0 params, got {}",
                                callcode_param_count
                            );
                        }
                        // Keep CALLCODE ABI consistent with normal CALL while preserving caller signature checks at the end of delegated tail execution. NOTE: Fitsh-compiled functions may POP one argv slot in normal paths. Even when CALLCODE target declares 0 params, injecting Nil here avoids accidental pop-empty-stack in delegated bodies written as regular funcs.
                        let caller_types = curr_ref!().types.clone();
                        Self::prepare_frame(
                            curr!(),
                            fnptr.mode,
                            true,
                            fnobj,
                            height,
                            Some(Value::Nil),
                        )?;
                        curr!().ret_check_policy = match caller_types {
                            Some(types) => RetCheckPolicy::CallcodeCallerRetContract(types),
                            None => RetCheckPolicy::CallcodeCallerNoRetContract,
                        };
                        continue;
                    }

                    // Check public access for outer calls
                    if let Outer = fnptr.mode {
                        if !fn_is_public {
                            let vis = plan.visibility_addr();
                            let owner = plan.code_owner();
                            let impl_in = maybe!(vis == owner, s!(""), 
                                format!(" (impl in {})", owner.to_readable()));
                            return itr_err_fmt!(
                                CallNotPublic,
                                "contract {}{} func sign {}",
                                vis.to_readable(),
                                impl_in,
                                fnptr.fnsign.to_hex()
                            );
                        }
                    }

                    // Create new frame for normal calls
                    let param = Some(curr!().pop_value()?);
                    let next = self.increase(r)?;
                    self.push(next);
                    Self::prepare_frame(curr!(), fnptr.mode, false, fnobj, height, param)?;

                    // plan_call already enforces mode/address contract; frame applies result directly.
                    match plan {
                        DispatchPlan::KeepState { code_owner, .. } => {
                            debug_assert!(matches!(fnptr.mode, Inner | View | Pure));
                            curr!().code_owner = code_owner;
                        }
                        DispatchPlan::SwitchState {
                            state_addr,
                            code_owner,
                            ..
                        } => {
                            debug_assert!(matches!(fnptr.mode, Outer));
                            curr!().state_addr = state_addr;
                            curr!().code_owner = code_owner;
                        }
                    }
                }

                Abort | Throw | Finish | Return => {
                    // Extract return value
                    let mut retv = Value::Nil;
                    if matches!(exit, Return | Throw) {
                        retv = curr!().pop_value()?;
                    }
                    // Error exits must bypass output-type validation and bubble original throw/abort semantics.
                    if matches!(exit, Abort | Throw) {
                        return itr_err_fmt!(ThrowAbort, "VM return error: {}", retv);
                    }
                    let ret_policy = std::mem::replace(
                        &mut curr!().ret_check_policy,
                        RetCheckPolicy::NonCallcode,
                    );
                    match ret_policy {
                        RetCheckPolicy::NonCallcode => curr!().check_output_type(&mut retv)?,
                        // CALLCODE without caller return contract keeps only base return-value validity checks.
                        RetCheckPolicy::CallcodeCallerNoRetContract => retv.canbe_func_retv()?,
                        // CALLCODE with caller return contract must follow caller contract, not callee's.
                        RetCheckPolicy::CallcodeCallerRetContract(caller_types) => {
                            retv.canbe_func_retv()?;
                            caller_types.check_output(&mut retv)?;
                        }
                    }
                    // Pop current frame and reclaim resources
                    self.pop().unwrap().reclaim(r);

                    // Bubble return through tail calls
                    loop {
                        let is_tail = match self.frames.last() {
                            Some(f) => f.pc == f.codes.len(),
                            None => return Ok(retv),
                        };

                        if !is_tail {
                            curr!().push_value(retv)?;
                            break;
                        }

                        // Tail-call collapse
                        curr_ref!().check_output_type(&mut retv)?;
                        self.pop().unwrap().reclaim(r);
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod owner_resolution_tests {
    use super::*;
    use field::{Address, Uint4};
    use std::sync::Arc;

    fn mk_contract_addr(n: u32) -> ContractAddress {
        let base = Address::from_readable("1MzNY1oA3kfgYi75zquj3SRUPYztzXHzK9").unwrap();
        ContractAddress::calculate(&base, &Uint4::from(n))
    }

    #[test]
    fn dispatch_plan_visibility_addr_prefers_state_addr_when_present() {
        let state_addr = mk_contract_addr(1);
        let owner = mk_contract_addr(2);
        let plan = DispatchPlan::SwitchState {
            state_addr: state_addr.clone(),
            code_owner: owner,
            fnobj: Arc::new(FnObj::plain(CodeType::Bytecode, vec![], 0, None)),
        };
        assert_eq!(plan.visibility_addr(), &state_addr);
    }

    #[test]
    fn dispatch_plan_visibility_addr_uses_owner_for_keep_state() {
        let owner = mk_contract_addr(3);
        let plan = DispatchPlan::KeepState {
            code_owner: owner.clone(),
            fnobj: Arc::new(FnObj::plain(CodeType::Bytecode, vec![], 0, None)),
        };
        assert_eq!(plan.visibility_addr(), &owner);
    }

    #[test]
    fn dispatch_plan_into_parts_roundtrip() {
        let state_addr = mk_contract_addr(4);
        let owner = mk_contract_addr(5);
        let plan = DispatchPlan::SwitchState {
            state_addr: state_addr.clone(),
            code_owner: owner.clone(),
            fnobj: Arc::new(FnObj::plain(CodeType::Bytecode, vec![], 0, None)),
        };
        let (next_state, next_owner, _fnobj) = plan.into_parts();
        assert_eq!(next_state, Some(state_addr));
        assert_eq!(next_owner, owner);
    }
}
