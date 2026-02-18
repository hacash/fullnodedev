                        // println!("CALLCODE() state_addr={}, code_owner={}", state_addr.prefix(7), code_owner.prefix(7));

fn resolve_non_outer_code_owner(
    target: &CallTarget,
    next_code_owner: Option<ContractAddress>,
    state_addr: &ContractAddress,
    code_owner: &ContractAddress,
) -> VmrtRes<ContractAddress> {
    match target {
        CallTarget::Libidx(_) => {
            // Invariant: lib-index lookup must always produce concrete code owner.
            let Some(owner) = next_code_owner else {
                return itr_err_fmt!(CallNotExist, "libidx call target missing code owner");
            };
            Ok(owner)
        }
        CallTarget::This => Ok(next_code_owner.unwrap_or(state_addr.clone())),
        CallTarget::Self_ | CallTarget::Super => Ok(next_code_owner.unwrap_or(code_owner.clone())),
    }
}

fn resolve_outer_frame_addrs(
    next_state_addr: Option<ContractAddress>,
    next_code_owner: Option<ContractAddress>,
) -> VmrtRes<(ContractAddress, ContractAddress)> {
    let Some(state_addr) = next_state_addr else {
        return itr_err_fmt!(CallNotExist, "outer call target missing state address");
    };
    let owner = next_code_owner.unwrap_or_else(|| state_addr.clone());
    Ok((state_addr, owner))
}

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

    pub fn start_call(&mut self, r: &mut Resoure, env: &mut ExecEnv, mode: ExecMode, code: &FnObj,
        entry_addr: ContractAddress,
        code_owner: Option<ContractAddress>,
        libs: Option<Vec<ContractAddress>>,
        param: Option<Value>
    ) -> VmrtRes<Value> {
        // Macro definitions for frame access
        macro_rules! curr { () => { self.frames.last_mut().unwrap() } }
        macro_rules! curr_ref { () => { self.frames.last().unwrap() } }
        
        use CallExit::*;
        use ExecMode::*;
        
        let libs_none: Option<Vec<ContractAddress>> = None;
        let height = env.ctx.env().block.height;
        
        // Setup root frame (depth=0, nested frames get depth+1 via Frame::next)
        self.contract_count = r.contracts.len();
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
                        curr_ref!().depth
                    );

                    let libs_ptr = if depth == 0 { &libs } else { &libs_none };
                    let loaded = r.load_must_call(env.ctx, fnptr.clone(), &state_addr, &code_owner, libs_ptr)?;
                    let next_state_addr = loaded.state_addr;
                    let next_code_owner = loaded.code_owner;
                    let fnobj_arc = loaded.fnobj;
                    let fnobj = fnobj_arc.as_ref();
                    let fn_is_public = fnobj.check_conf(FnConf::Public);
                    self.check_load_new_contract_and_gas(r, env)?;
                    
                    // CALLCODE: in-place execution
                    if fnptr.is_callcode {
                        let owner = next_code_owner.as_ref().cloned().unwrap_or_else(|| state_addr.clone());
                        curr!().code_owner = owner;
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
                        Self::prepare_frame(curr!(), fnptr.mode, true, fnobj, height, Some(Value::Nil))?;
                        curr!().callcode_caller_types = caller_types;
                        continue;
                    }
                    
                    // Check public access for outer calls
                    if let Outer = fnptr.mode {
                        if !fn_is_public {
                            let Some(cadr) = next_state_addr.as_ref().or(next_code_owner.as_ref()) else {
                                return itr_err_fmt!(
                                    CallNotExist,
                                    "outer call target missing address for visibility check"
                                );
                            };
                            return itr_err_fmt!(CallNotPublic, "contract {} func sign {}", cadr.to_readable(), fnptr.fnsign.to_hex());
                        }
                    }
                    
                    // Create new frame for normal calls
                    let param = Some(curr!().pop_value()?);
                    let next = self.increase(r)?;
                    self.push(next);
                    Self::prepare_frame(curr!(), fnptr.mode, false, fnobj, height, param)?;
                    
                    // Set context addresses based on call mode
                    match fnptr.mode {
                        Inner | View | Pure => {
                            // Non-Outer calls keep storage context inherited by Frame::next(). Only dispatch owner can change (this/self/super or lib lookup). CALLCODE may rewrite code_owner in-place, but call instructions are blocked while in_callcode=true (check_call_mode), so this branch always handles normal nested frames.
                            let owner = resolve_non_outer_code_owner(
                                &fnptr.target,
                                next_code_owner,
                                &state_addr,
                                &code_owner,
                            )?;
                            curr!().code_owner = owner;
                        }
                        Outer => {
                            let (target_state_addr, owner) =
                                resolve_outer_frame_addrs(next_state_addr, next_code_owner)?;
                            curr!().state_addr = target_state_addr;
                            curr!().code_owner = owner;
                        }
                        _ => unreachable!()
                    }
                }
                
                Abort | Throw | Finish | Return => {
                    // Extract return value
                    let mut retv = Value::Nil;
                    if matches!(exit, Return | Throw) {
                        retv = curr!().pop_value()?;
                    }
                    if let Some(caller_types) = curr!().callcode_caller_types.take() {
                        // CALLCODE is treated as implementation-level delegation: only the original caller's return contract is enforced here.
                        caller_types.check_output(&mut retv)?;
                    } else {
                        curr!().check_output_type(&mut retv)?;
                    }
                    
                    // Handle abort/throw
                    if matches!(exit, Abort | Throw) {
                        return itr_err_fmt!(ThrowAbort, "VM return error: {}", retv);
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


    fn check_load_new_contract_and_gas(&mut self, r: &mut Resoure, env: &mut ExecEnv) -> VmrtErr {
        let ctlnum = &mut self.contract_count;
        // check gas
        let ctln = r.contracts.len();
        let delta = ctln.saturating_sub(*ctlnum);
        if delta > 0 || r.contract_load_bytes > 0 {
            // Library resolve may touch src+lib (usually 1-2 loads), while inheritance resolve can walk multiple parents, so delta can be >1 in a single CALL.
            let fee = (delta as i64) * r.gas_extra.load_new_contract;
            let bytes_fee = (r.contract_load_bytes as i64) / 64;
            *env.gas -= fee + bytes_fee;
            r.contract_load_bytes = 0;
            if *env.gas < 0 {
                return itr_err_code!(OutOfGas)
            }
            // update count
            *ctlnum = ctln;
        }
        Ok(())
    }
    

}

#[cfg(test)]
mod gas_tests {
    use super::*;
    use basis::component::Env;
    use basis::interface::Context;
    use std::sync::Arc;
    use testkit::sim::context::make_ctx_with_state;
    use testkit::sim::state::FlatMemState as StateMem;
    use testkit::sim::tx::DummyTx;

    #[test]
    fn contract_load_gas_charges_base_plus_bytes_div_64() {
        let tx = DummyTx::default();
        let mut env = Env::default();
        env.block.height = 1;
        let mut ctx = make_ctx_with_state(env, Box::new(StateMem::default()), &tx);
        let ctx: &mut dyn Context = &mut ctx;

        let mut gas = 1000i64;
        let mut exenv = ExecEnv { ctx, gas: &mut gas };

        let mut r = Resoure::create(1);
        r.contract_load_bytes = 129; // bytes_fee = 2
        r.contracts
            .insert(ContractAddress::default(), Arc::new(ContractObj::default())); // delta=1

        let mut call = CallFrame::new();
        call.contract_count = 0;

        call.check_load_new_contract_and_gas(&mut r, &mut exenv).unwrap();

        // fee = 1 * 32 + 129/64 = 32 + 2 = 34
        assert_eq!(gas, 966);
        assert_eq!(r.contract_load_bytes, 0);
        assert_eq!(call.contract_count, 1);
    }

}

#[cfg(test)]
mod owner_resolution_tests {
    use super::*;
    use field::{Address, Uint4};

    fn mk_contract_addr(n: u32) -> ContractAddress {
        let base = Address::from_readable("1MzNY1oA3kfgYi75zquj3SRUPYztzXHzK9").unwrap();
        ContractAddress::calculate(&base, &Uint4::from(n))
    }

    #[test]
    fn libidx_requires_loader_owner() {
        let state_addr = mk_contract_addr(1);
        let code_owner = mk_contract_addr(2);
        let err = resolve_non_outer_code_owner(
            &CallTarget::Libidx(0),
            None,
            &state_addr,
            &code_owner,
        )
        .unwrap_err();
        assert_eq!(err.0, ItrErrCode::CallNotExist);
    }

    #[test]
    fn libidx_prefers_loader_owner() {
        let state_addr = mk_contract_addr(1);
        let code_owner = mk_contract_addr(2);
        let loader_owner = mk_contract_addr(3);
        let got = resolve_non_outer_code_owner(
            &CallTarget::Libidx(0),
            Some(loader_owner.clone()),
            &state_addr,
            &code_owner,
        )
        .unwrap();
        assert_eq!(got, loader_owner);
    }

    #[test]
    fn this_falls_back_to_state_addr() {
        let state_addr = mk_contract_addr(1);
        let code_owner = mk_contract_addr(2);
        let got = resolve_non_outer_code_owner(
            &CallTarget::This,
            None,
            &state_addr,
            &code_owner,
        )
        .unwrap();
        assert_eq!(got, state_addr);
    }

    #[test]
    fn self_falls_back_to_current_code_owner() {
        let state_addr = mk_contract_addr(1);
        let code_owner = mk_contract_addr(2);
        let got = resolve_non_outer_code_owner(
            &CallTarget::Self_,
            None,
            &state_addr,
            &code_owner,
        )
        .unwrap();
        assert_eq!(got, code_owner);
    }

    #[test]
    fn outer_requires_state_addr() {
        let err = resolve_outer_frame_addrs(None, Some(mk_contract_addr(2))).unwrap_err();
        assert_eq!(err.0, ItrErrCode::CallNotExist);
    }

    #[test]
    fn outer_falls_back_owner_to_state_addr() {
        let state_addr = mk_contract_addr(1);
        let (resolved_state, resolved_owner) =
            resolve_outer_frame_addrs(Some(state_addr.clone()), None).unwrap();
        assert_eq!(resolved_state, state_addr);
        assert_eq!(resolved_owner, state_addr);
    }
}
