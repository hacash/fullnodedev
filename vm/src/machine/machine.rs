

#[allow(dead_code)]
pub struct MachineBox {
    gas: i64,
    gas_price: i64,
    machine: Option<Machine>
} 

impl Drop for MachineBox {
    fn drop(&mut self) {
        // println!("\n---------------\n[MachineBox Drop] Reclaim resoure))\n---------------\n");
        match self.machine.take() {
            Some(m) => global_machine_manager().reclaim(m.r),
            _ => ()
        }
    }
}

impl MachineBox {
    
    pub fn new(m: Machine) -> Self {
        Self { 
            gas: i64::MIN, // init in first call
            gas_price: 0,
            machine: Some(m)
        }
    }

    fn check_gas(&mut self, ctx: &mut dyn Context) -> Rerr {        
        const L: i64 = i64::MIN;
        match self.gas {
            L     => self.init_gas(ctx),
            L..=0 => errf!("gas has run out"),
            _     => Ok(()) // gas > 0
        }
    }

    fn init_gas(&mut self, ctx: &mut dyn Context) -> Rerr {
        // init gas
        let gascp = &self.machine.as_mut().unwrap().r.gas_extra;
        let gas_limit = gascp.max_gas_of_tx;
        let (feer, gasfee) = ctx.tx().fee_extend()?;
        if feer == 0 {
            return errf!("gas extend cannot empty on contract call")
        }
        let main = ctx.env().tx.main;
        protocol::operate::hac_check(ctx, &main, &gasfee)?;
        let mut gas = ctx.tx().size() as i64 * feer as i64;
        up_in_range!(gas, 0, gas_limit);  // max 65535
        self.gas = gas;
        self.gas_price = Self::gas_price(ctx);
        Ok(())
    }

    fn check_cost(&self, cty: ExecMode, mut cost: i64) -> Ret<i64> {
        use ExecMode::*;
        assert!(cost > 0, "gas cost error");
        // min use
        let gsext = &self.machine.as_ref().unwrap().r.gas_extra;
        let min_use = match cty {
            Main | P2sh => gsext.main_call_min,
            Abst => gsext.abst_call_min,
            _ => never!()
        };
        up_in_range!(cost, min_use, i64::MAX);
        Ok(cost)
    }


    fn spend_gas(&self, ctx: &mut dyn Context, cost: i64) -> Rerr {
        assert!(self.gas_price > 0, "gas price error");
        // do spend
        let cost_per = cost * (self.gas_price / GSCU as i64);
        assert!(cost_per > 0, "gas cost error");
        let cost_amt = Amount::unit238(cost_per as u64);
        let main = ctx.env().tx.main;
        protocol::operate::hac_sub(ctx, &main, &cost_amt)?;
        Ok(())
    }

    fn gas_price(ctx: &dyn Context) -> i64 {
        let gs = ctx.tx().fee_purity() as i64;
        gs // calc by fee got
    }


}

impl VM for MachineBox {
    fn usable(&self) -> bool { true }
    fn call(&mut self, 
        ctx: &mut dyn Context, sta: &mut dyn State,
        ty: u8, kd: u8, data: &[u8], param: Box<dyn Any>
    ) -> Ret<(i64, Vec<u8>)> {
        use ExecMode::*;
        // init gas & check balance
        self.check_gas(ctx)?;
        let gas = &mut self.gas;
        let gas_record = *gas;
        // env & do call
        let machine = self.machine.as_mut().unwrap();
        let not_in_calling = ! machine.is_in_calling();
        let sta = &mut VMState::wrap(sta);
        let exenv = &mut ExecEnv{ ctx, sta, gas };
        let cty: ExecMode = std_mem_transmute!(ty);
        let resv = match cty {
            Main => {
                let cty = CodeType::parse(kd)?;
                machine.main_call(exenv, cty, data.to_vec())
            }
            P2sh => {
                let (ctxadr, mv1) = Address::create(data)?;
                let (calibs, mv2) = ContractAddressW1::create(&data[mv1..])?;
                let mv = mv1 + mv2;
                let realcodes = data[mv..].to_vec();
                let Ok(param) = param.downcast::<Value>() else {
                    return errf!("p2sh argv type not match")
                };
                machine.p2sh_call(exenv, ctxadr, calibs.into_list(), realcodes, *param)
            }
            Abst => {
                let kid: AbstCall = std_mem_transmute!(kd);
                let cadr = ContractAddress::parse(data)?;
                let Ok(param) = param.downcast::<Value>() else {
                    return errf!("abst argv type not match")
                };
                machine.abst_call(exenv, kid, cadr, *param)
            }
            _ => unreachable!()
        }.map(|a|a.raw())?;
        let gas_current = *gas;
        let mut cost = gas_record - gas_current;
        cost = self.check_cost(cty, cost)?;
        // spend gas, but in calling do not spend
        if not_in_calling {
            self.spend_gas(ctx, cost)?;
        }
        // ok
        Ok((cost, resv))
    }
}




/*********************************/





#[allow(dead_code)]
pub struct Machine {
    r: Resoure,
    frames: Vec<CallFrame>,
}



impl Machine {

    pub fn is_in_calling(&self) -> bool {
        ! self.frames.is_empty()
    }

    pub fn create(r: Resoure) -> Self {
        Self {
            r,
            frames: vec![],
        }
    }

    pub fn main_call(&mut self, env: &mut ExecEnv, ctype: CodeType, codes: Vec<u8>) -> Ret<Value> {
        let fnobj = FnObj{ ctype, codes, confs: 0, agvty: None};
        let ctx_adr = ContractAddress::from_unchecked(env.ctx.tx().main());
        let lib_adr = env.ctx.env().tx.addrs.iter().map(|a|ContractAddress::from_unchecked(*a)).collect();
        let rv = self.do_call(env, ExecMode::Main, fnobj, ctx_adr, None, Some(lib_adr), None)?;
        Ok(rv)
    }

    pub fn abst_call(&mut self, env: &mut ExecEnv, cty: AbstCall, contract_addr: ContractAddress, param: Value) -> Ret<Value> {
        let adr = contract_addr.readable();
        let Some((owner, fnobj)) = self.r.load_abstfn(env.sta, &contract_addr, cty)? else {
            // return Ok(Value::Nil) // not find call
            return errf!("abst call {:?} not find in {}", cty, adr) // not find call
        };
        let fnobj = fnobj.as_ref().clone();
        let rv = self.do_call(env, ExecMode::Abst, fnobj, contract_addr, owner, None, Some(param))?;
        if rv.check_true() {
            return errf!("call {}.{:?} return error code {}", adr, cty, rv.to_uint())
        }
        Ok(rv)
    }

    fn p2sh_call(&mut self, env: &mut ExecEnv, p2sh_addr: Address, libs: Vec<ContractAddress>, codes: Vec<u8>, param: Value) -> Ret<Value> {
        let ctype = CodeType::Bytecode;
        let fnobj = FnObj{ ctype, codes, confs: 0, agvty: None};
        let ctx_adr = ContractAddress::from_unchecked(p2sh_addr);
        let rv = self.do_call(env, ExecMode::P2sh, fnobj, ctx_adr, None, Some(libs), Some(param))?;
        if rv.check_true() {
            return errf!("p2sh call return error code {}", rv.to_uint())
        }
        Ok(rv)
    }

    fn do_call(&mut self, env: &mut ExecEnv, mode: ExecMode, code: FnObj, entry_addr: ContractAddress, code_owner: Option<ContractAddress>, libs: Option<Vec<ContractAddress>>, param: Option<Value>) -> VmrtRes<Value> {
        self.frames.push(CallFrame::new()); // for reclaim
        let res = self.frames.last_mut().unwrap().start_call(&mut self.r, env, mode, code, entry_addr, code_owner, libs, param);
        self.frames.pop().unwrap().reclaim(&mut self.r); // do reclaim
        res
    }



}


#[cfg(test)]
mod machine_test {

    use super::*;
    use crate::contract::{Contract, Func};
    use crate::lang::lang_to_bytecode;
    use crate::rt::CodeType;

    use basis::component::Env;
    use basis::interface::{Context, State, TransactionRead};
    use field::{Address, Amount, Hash, Uint4};
    use protocol::context::{ContextInst, EmptyState};
    use protocol::state::EmptyLogs;

    #[derive(Default, Clone)]
    struct StateMem {
        mem: basis::component::MemKV,
    }

    impl State for StateMem {
        fn get(&self, k: Vec<u8>) -> Option<Vec<u8>> {
            match self.mem.get(&k) {
                Some(v) => v.clone(),
                None => None,
            }
        }
        fn set(&mut self, k: Vec<u8>, v: Vec<u8>) {
            self.mem.put(k, v)
        }
        fn del(&mut self, k: Vec<u8>) {
            self.mem.del(k)
        }
    }

    #[derive(Clone, Debug)]
    struct DummyTx {
        main: Address,
        addrs: Vec<Address>,
    }

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
            0
        }
        fn hash(&self) -> Hash {
            Hash::default()
        }
        fn hash_with_fee(&self) -> Hash {
            Hash::default()
        }
        fn main(&self) -> Address {
            self.main
        }
        fn addrs(&self) -> Vec<Address> {
            self.addrs.clone()
        }
        fn fee(&self) -> &Amount {
            Amount::zero_ref()
        }
        fn fee_purity(&self) -> u64 {
            1
        }
        fn fee_extend(&self) -> Ret<(u16, Amount)> {
            Ok((1, Amount::zero()))
        }
    }

    #[test]
    fn calltargets_resolve_under_callview_and_inherits() {
        // Arrange addresses.
        let base_addr = Address::from_readable("1MzNY1oA3kfgYi75zquj3SRUPYztzXHzK9").unwrap();
        let contract_child = crate::ContractAddress::calculate(&base_addr, &Uint4::from(1));
        let contract_parent = crate::ContractAddress::calculate(&base_addr, &Uint4::from(2));
        let contract_base = crate::ContractAddress::calculate(&base_addr, &Uint4::from(3));

        // Build an inheritance chain: Child -> Parent -> Base.
        // The key trick is: `super.f()` moves curadr (code owner) to Parent, while ctxadr stays Child.
        // Then inside Parent.f(), `this.g()` must resolve in ctxadr (Child), `self.g()` in curadr (Parent),
        // and `super.g()` in Parent's direct base (Base).

        let base = Contract::new().func(Func::new("g").fitsh("return 3").unwrap());

        let parent = Contract::new()
            .inh(contract_base.to_addr())
            .func(Func::new("g").fitsh("return 2").unwrap())
            .func(
                Func::new("f")
                    .fitsh(
                        r##"
                        return this.g() * 10000 + self.g() * 100 + super.g()
                        "##,
                    )
                    .unwrap(),
            );

        let child = Contract::new()
            .inh(contract_parent.to_addr())
            .func(Func::new("g").fitsh("return 1").unwrap())
            .func(
                Func::new("run")
                    .public()
                    .fitsh(
                        r##"
                        return super.f()
                        "##,
                    )
                    .unwrap(),
            );

        // Put contracts into VM state.
        let mut ext_state = StateMem::default();
        let mut vmsta = crate::VMState::wrap(&mut ext_state);
        vmsta.contract_set(&contract_base, &base.into_sto());
        vmsta.contract_set(&contract_parent, &parent.into_sto());
        vmsta.contract_set(&contract_child, &child.into_sto());

        // Main script calls contract_main.run() using tx-provided libs (index 0).
        let main_script = r##"
            lib C = 0
            return C.run()
        "##;
        let main_codes = lang_to_bytecode(main_script).unwrap();

        let mut env = Env::default();
        env.block.height = 1;
        env.tx.main = base_addr;
        env.tx.addrs = vec![contract_child.into_addr()];

        let tx = DummyTx {
            main: base_addr,
            addrs: env.tx.addrs.clone(),
        };
        let mut ctx = ContextInst::new(env, Box::new(EmptyState {}), Box::new(EmptyLogs {}), &tx);

        let mut gas: i64 = 1_000_000;
        let mut exec = crate::frame::ExecEnv {
            ctx: &mut ctx as &mut dyn Context,
            sta: &mut vmsta,
            gas: &mut gas,
        };

        let mut machine = Machine::create(Resoure::create(1));
        let rv = machine
            .main_call(&mut exec, CodeType::Bytecode, main_codes)
            .unwrap();

        // Expected: inside Parent.f(), ctxadr=Child, curadr=Parent.
        // this.g()=1 (Child), self.g()=2 (Parent), super.g()=3 (Base).
        assert_eq!(rv.to_uint(), 10_203);
    }

/*
    i64::MAX  = 9223372036854775807
    10000 HAC =   10000000000000000:236

    0.00000001 = 1:240 = 10000:236




*/


}
