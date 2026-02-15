                        // println!("CALLCODE() ctxadr={}, curadr={}", ctxadr.prefix(7), curadr.prefix(7));

impl CallFrame {

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
                    let fnobj = fnobj.as_ref();
                    let fn_is_public = fnobj.check_conf(FnConf::Public);
                    self.check_load_new_contract_and_gas(r, env)?;
                    
                    // CALLCODE: in-place execution
                    if fnptr.is_callcode {
                        let owner = chgsrcadr.as_ref().cloned().unwrap_or_else(|| ctxadr.clone());
                        curr!().curadr = owner;
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

#[cfg(test)]
mod gas_tests {
    use super::*;
    use basis::component::Env;
    use basis::interface::{Context, State, TransactionRead};
    use field::{Address, Amount, Hash};
    use protocol::context::ContextInst;
    use protocol::state::EmptyLogs;
    use std::collections::HashMap;
    use std::sync::Arc;
    use sys::Ret;

    #[derive(Default, Clone, Debug)]
    struct DummyTx;

    impl field::Serialize for DummyTx {
        fn size(&self) -> usize {
            0
        }
        fn serialize(&self) -> Vec<u8> {
            vec![]
        }
    }

    impl basis::interface::TxExec for DummyTx {}

    impl TransactionRead for DummyTx {
        fn ty(&self) -> u8 {
            3
        }
        fn hash(&self) -> Hash {
            Hash::default()
        }
        fn hash_with_fee(&self) -> Hash {
            Hash::default()
        }
        fn main(&self) -> Address {
            Address::default()
        }
        fn addrs(&self) -> Vec<Address> {
            vec![Address::default()]
        }
        fn fee(&self) -> &Amount {
            Amount::zero_ref()
        }
        fn fee_purity(&self) -> u64 {
            1
        }
        fn fee_extend(&self) -> Ret<u8> {
            Ok(1)
        }
    }

    #[derive(Default)]
    struct StateMem {
        mem: HashMap<Vec<u8>, Vec<u8>>,
    }

    impl State for StateMem {
        fn get(&self, k: Vec<u8>) -> Option<Vec<u8>> {
            self.mem.get(&k).cloned()
        }
        fn set(&mut self, k: Vec<u8>, v: Vec<u8>) {
            self.mem.insert(k, v);
        }
        fn del(&mut self, k: Vec<u8>) {
            self.mem.remove(&k);
        }
    }

    #[test]
    fn contract_load_gas_charges_base_plus_bytes_div_64() {
        let tx = DummyTx::default();
        let mut env = Env::default();
        env.block.height = 1;
        let mut ctx = ContextInst::new(
            env,
            Box::new(StateMem::default()),
            Box::new(EmptyLogs {}),
            &tx,
        );
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
