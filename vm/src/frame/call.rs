                        // println!("CALLCODE() ctxadr={}, curadr={}", ctxadr.prefix(7), curadr.prefix(7));

impl CallFrame {

    pub fn start_call_old(&mut self, r: &mut Resoure, env: &mut ExecEnv, mode: ExecMode, code: FnObj, 
        entry_addr: ContractAddress, 
        code_owner: Option<ContractAddress>,
        libs: Option<Vec<ContractAddress>>, 
        param: Option<Value>
    ) -> VmrtRes<Value> {
        macro_rules! curr { () => { self.frames.last_mut().unwrap() }}
        macro_rules! curr_ref { () => { self.frames.last().unwrap() }}
        macro_rules! with_curr {
            (|$curr:ident| $body:block) => {{
                let $curr = self.frames.last_mut().unwrap();
                $body
            }};
        }
        use CallExit::*;
        use ExecMode::*;
        let libs_none: Option<Vec<ContractAddress>> = None;
        // to spend gas
        self.contract_count = r.contracts.len();
        let mut curf = self.increase(r)?; // current frame
        // Root frame depth from ctx.depth (0=Main/P2sh, 1=Abst). Nested CALLs use Frame::next().
        curf.depth = env.ctx.depth().to_isize() as isize;
        curf.ctxadr = entry_addr.clone();
        curf.curadr = code_owner.unwrap_or(entry_addr);
        self.push(curf);
        // compile irnode and push func argv ...
        curr!().prepare(mode, false, code, param)?;
        // exec codes
        loop {
            let exit = { curr!().execute(r, env)? }; // call frame
            match exit {
                // end func
                Abort | Throw | Finish | Return => {
                    let mut retv = Value::Nil;
                    with_curr!(|curr| {
                        if matches!(exit, Return | Throw) {
                            retv = curr.pop_value()?;
                        }
                        curr.check_output_type(&mut retv)?;
                    });
                    match exit {
                        Abort | Throw => return itr_err_fmt!(ThrowAbort, "VM return error: {}", retv),
                        Finish | Return => {
                            let curr = self.pop().unwrap();
                            curr.reclaim(r); // reclaim resource
                            loop {
                                let is_tail = match self.frames.last() {
                                    Some(prev) => prev.pc == prev.codes.len(),
                                    None => return Ok(retv), // all call finish
                                };
                                if !is_tail {
                                    self.frames.last_mut().unwrap()
                                        .push_value(retv)?; // push func call result
                                    break;
                                }
                                // tail-call collapse: return directly to upper frame
                                with_curr!(|prev| {
                                    prev.check_output_type(&mut retv)?;
                                });
                                let prev = self.pop().unwrap();
                                prev.reclaim(r);
                            }
                            continue // prev frame do execute
                        }
                        _ => unreachable!()
                    }
                }
                // next call
                Call(fnptr) => {
                    let (ctxadr, curadr, depth) = {
                        let curr = curr_ref!();
                        (curr.ctxadr.clone(), curr.curadr.clone(), curr.depth)
                    };
                    // depth==0: entry layer (Main/P2sh) uses tx libs; depth>0: nested/Abst uses no libs
                    let libs_ptr = maybe!(depth == 0, &libs, &libs_none);
                    let (chgsrcadr, fnobj) = r.load_must_call(env.ctx, fnptr.clone(), 
                        &ctxadr, &curadr, libs_ptr)?;
                    let fnobj = fnobj.as_ref().clone();
                    let fn_is_public = fnobj.check_conf(FnConf::Public);
                    // check gas
                    self.check_load_new_contract_and_gas(r, env)?;
                    // if call code
                    if fnptr.is_callcode {
                        // CALLCODE: execute in current frame, depth unchanged, inherits mode permissions
                        // println!("CALLCODE() ctxadr={}, curadr={}", ctxadr.prefix(7), curadr.prefix(7));
                        let owner = chgsrcadr.as_ref().cloned().unwrap_or_else(|| ctxadr.clone());
                        curr!().curadr = owner;
                        curr!().prepare(fnptr.mode, true, fnobj, None)?; // no param
                        continue // do execute
                    }
                    if let Outer = fnptr.mode {
                        let cadr = chgsrcadr.as_ref().unwrap();
                        if ! fn_is_public {
                            return itr_err_fmt!(CallNotPublic, "contract {} func sign {}", cadr.to_readable(), fnptr.fnsign.to_hex())
                        }
                    }
                    // call next frame (nested CALL: increase() uses Frame::next() which sets depth = parent.depth + 1)
                    // println!("{:?}() ctxadr={}, curadr={}", fnptr.mode, ctxadr.prefix(7), curadr.prefix(7));
                    let param = Some(curr!().pop_value()?);
                    let next = self.increase(r)?;
                    self.push(next);
                    curr!().prepare(fnptr.mode, false, fnobj, param)?;
                    match fnptr.mode {
                        Inner | View | Pure => {
                            // curadr follows resolved code owner (child/parent or library)
                            let default_owner = match fnptr.target {
                                CallTarget::This => ctxadr.clone(),
                                CallTarget::Self_ | CallTarget::Super => curadr.clone(),
                                CallTarget::Libidx(_) => ctxadr.clone(),
                            };
                            let owner = chgsrcadr.as_ref().cloned().unwrap_or(default_owner);
                            curr!().curadr = owner;
                            // continue to do next call
                        }
                        Outer => {
                            let cadr = chgsrcadr.unwrap();
                            with_curr!(|curr| {
                                curr.ctxadr = cadr.clone(); 
                                curr.curadr = cadr; 
                            });
                            // continue to do next call
                        }
                        _ => unreachable!()
                    }
                    continue
                }
            }
            // panic!("unreachable exit {:?}", exit);
            // unreachable!()
        }
    }


    pub fn start_call_old2(&mut self, r: &mut Resoure, env: &mut ExecEnv, mode: ExecMode, code: FnObj,
        entry_addr: ContractAddress,
        code_owner: Option<ContractAddress>,
        libs: Option<Vec<ContractAddress>>,
        param: Option<Value>
    ) -> VmrtRes<Value> {
        macro_rules! curr { () => { self.frames.last_mut().unwrap() } }
        macro_rules! curr_ref { () => { self.frames.last().unwrap() } }
        use CallExit::*;
        use ExecMode::*;
        let libs_none: Option<Vec<ContractAddress>> = None;
        self.contract_count = r.contracts.len();
        let mut root = self.increase(r)?;
        root.depth = env.ctx.depth().to_isize() as isize;
        root.ctxadr = entry_addr.clone();
        root.curadr = code_owner.unwrap_or(entry_addr);
        self.push(root);
        // Root frame enters execution with the entry function prepared.
        curr!().prepare(mode, false, code, param)?;
        loop {
            let exit = curr!().execute(r, env)?;
            match exit {
                Call(fnptr) => {
                    let (ctxadr, curadr, depth) = {
                        let curr = curr_ref!();
                        (curr.ctxadr.clone(), curr.curadr.clone(), curr.depth)
                    };
                    let libs_ptr = maybe!(depth == 0, &libs, &libs_none);
                    let (chgsrcadr, fnobj) = r.load_must_call(env.ctx, fnptr.clone(), &ctxadr, &curadr, libs_ptr)?;
                    let fnobj = fnobj.as_ref().clone();
                    let fn_is_public = fnobj.check_conf(FnConf::Public);
                    self.check_load_new_contract_and_gas(r, env)?;
                    if fnptr.is_callcode {
                        // CALLCODE keeps stack depth and rewires current code owner in-place.
                        let owner = chgsrcadr.as_ref().cloned().unwrap_or_else(|| ctxadr.clone());
                        curr!().curadr = owner;
                        curr!().prepare(fnptr.mode, true, fnobj, None)?;
                        continue;
                    }
                    if matches!(fnptr.mode, Outer) && !fn_is_public {
                        let cadr = chgsrcadr.as_ref().unwrap();
                        return itr_err_fmt!(CallNotPublic, "contract {} func sign {}", cadr.to_readable(), fnptr.fnsign.to_hex())
                    }
                    // Normal CALL pops args from caller and starts a new child frame.
                    let param = Some(curr!().pop_value()?);
                    let next = self.increase(r)?;
                    self.push(next);
                    curr!().prepare(fnptr.mode, false, fnobj, param)?;
                    if matches!(fnptr.mode, Outer) {
                        // OUTER call switches both context address and current address to callee.
                        let cadr = chgsrcadr.unwrap();
                        curr!().ctxadr = cadr.clone();
                        curr!().curadr = cadr;
                        continue;
                    }
                    // Inner/View/Pure call updates only current code owner for execution.
                    let default_owner = match fnptr.target {
                        CallTarget::This | CallTarget::Libidx(_) => ctxadr.clone(),
                        CallTarget::Self_ | CallTarget::Super => curadr.clone(),
                    };
                    curr!().curadr = chgsrcadr.unwrap_or(default_owner);
                }
                Abort | Throw | Finish | Return => {
                    let mut retv = Value::Nil;
                    {
                        let curr = curr!();
                        if matches!(exit, Return | Throw) {
                            retv = curr.pop_value()?;
                        }
                        curr.check_output_type(&mut retv)?;
                    }
                    if matches!(exit, Abort | Throw) {
                        return itr_err_fmt!(ThrowAbort, "VM return error: {}", retv)
                    }
                    // Finished frame is popped and its runtime resources are reclaimed.
                    self.pop().unwrap().reclaim(r);
                    loop {
                        let is_tail = match self.frames.last() {
                            Some(prev) => prev.pc == prev.codes.len(),
                            None => return Ok(retv),
                        };
                        if !is_tail {
                            // Return value is pushed back to the direct caller frame.
                            curr!().push_value(retv)?;
                            break;
                        }
                        // Tail-return collapse keeps bubbling value across completed callers.
                        curr_ref!().check_output_type(&mut retv)?;
                        self.pop().unwrap().reclaim(r);
                    }
                }
            }
        }
    }


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
        
        // Setup root frame
        self.contract_count = r.contracts.len();
        let mut root = self.increase(r)?;
        root.depth = env.ctx.depth().to_isize() as isize;
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
                        curr!().prepare(fnptr.mode, true, fnobj, None)?;
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
                    curr!().check_output_type(&mut retv)?;
                    
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
