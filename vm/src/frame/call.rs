                        // println!("CALLCODE() ctxadr={}, curadr={}", ctxadr.prefix(7), curadr.prefix(7));

impl CallFrame {

    pub fn start_call(&mut self, r: &mut Resoure, env: &mut ExecEnv, mode: ExecMode, code: FnObj,
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
        
        // Setup root frame (depth=0, nested frames get depth+1 via Frame::next)
        self.contract_count = r.contracts.len();
        let mut root = self.increase(r)?;
        root.ctxadr = entry_addr.clone();
        root.curadr = code_owner.unwrap_or(entry_addr);
        self.push(root);
        curr!().prepare(mode, false, code, param)?;

        // Main execution loop
        loop {
            let exit = curr!().execute(r, env)?;

            match exit {
                Call(fnptr) => {
                    // Load call context
                    let (ctxadr, curadr, depth) = (
                        curr_ref!().ctxadr.clone(),
                        curr_ref!().curadr.clone(),
                        curr_ref!().depth
                    );

                    let libs_ptr = if depth == 0 { &libs } else { &libs_none };
                    let (chgsrcadr, fnobj) = r.load_must_call(env.ctx, fnptr.clone(), &ctxadr, &curadr, libs_ptr)?;
                    let fnobj = fnobj.as_ref().clone();
                    let fn_is_public = fnobj.check_conf(FnConf::Public);
                    self.check_load_new_contract_and_gas(r, env)?;
                    
                    // CALLCODE: in-place execution
                    if fnptr.is_callcode {
                        let owner = chgsrcadr.as_ref().cloned().unwrap_or_else(|| ctxadr.clone());
                        curr!().curadr = owner;
                        let callcode_param_count = match &fnobj.agvty {
                            Some(types) => types.param_types().map_ire(CallArgvTypeFail)?.len(),
                            None => 0,
                        };
                        if callcode_param_count != 0 {
                            return itr_err_fmt!(
                                CallArgvTypeFail,
                                "callcode target must have 0 params, got {}",
                                callcode_param_count
                            );
                        }
                        // Keep CALLCODE ABI consistent with normal CALL while preserving caller
                        // signature checks at the end of delegated tail execution.
                        let caller_types = curr_ref!().types.clone();
                        curr!().prepare(fnptr.mode, true, fnobj, Some(Value::Nil))?;
                        curr!().callcode_caller_types = caller_types;
                        continue;
                    }
                    
                    // Check public access for outer calls
                    if let Outer = fnptr.mode {
                        let cadr = chgsrcadr.as_ref().unwrap();
                        if !fn_is_public {
                            return itr_err_fmt!(CallNotPublic, "contract {} func sign {}", cadr.to_readable(), fnptr.fnsign.to_hex());
                        }
                    }
                    
                    // Create new frame for normal calls
                    let param = Some(curr!().pop_value()?);
                    let next = self.increase(r)?;
                    self.push(next);
                    curr!().prepare(fnptr.mode, false, fnobj, param)?;
                    
                    // Set context addresses based on call mode
                    match fnptr.mode {
                        Inner | View | Pure => {
                            let default_owner = match fnptr.target {
                                CallTarget::This | CallTarget::Libidx(_) => ctxadr.clone(),
                                CallTarget::Self_ | CallTarget::Super => curadr.clone(),
                            };
                            curr!().curadr = chgsrcadr.unwrap_or(default_owner);
                        }
                        Outer => {
                            let cadr = chgsrcadr.unwrap();
                            curr!().ctxadr = cadr.clone();
                            curr!().curadr = cadr;
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
                        // CALLCODE is treated as implementation-level delegation:
                        // only the original caller's return contract is enforced here.
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
            // Library resolve may touch src+lib (usually 1-2 loads), while inheritance
            // resolve can walk multiple parents, so delta can be >1 in a single CALL.
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
