

/// Per-tx gas accounting state. Consolidates gas budget, fee caching,
/// re-entry depth tracking, and HAC burn settlement into one struct.
///
/// Gas price is based on "fee purity" — the miner-received fee per byte:
///   gas_price = fee_got / tx_size  (= fee_purity)
/// Settlement formula: burn_amt = ceil(cost * purity_fee / (purity_size * gas_rate))
/// We keep numerator/denominator separate to avoid integer division precision loss.
#[derive(Clone)]
pub struct GasCounter {
    pub remaining: i64,     // gas budget left (monotonic in one tx; never restored by AST recover)
    purity_fee: i128,       // fee purity numerator: fee_got in unit-238 (miner-received portion)
    purity_size: i128,      // fee purity denominator: tx serialized size in bytes
    gas_rate: i64,          // burn discount denominator (mainnet=1, L2 can be e.g. 10 or 32)
    initialized: bool,      // whether init_once() has been called
    reentry_depth: u32,     // current EXTACTION re-entry depth (0 = not in call)
    max_reentry: u32,       // hard cap from SpaceCap
}

struct GasCallGuard<'a> {
    account: &'a mut GasCounter,
}

impl<'a> GasCallGuard<'a> {
    fn enter(account: &'a mut GasCounter) -> Ret<Self> {
        account.enter()?;
        Ok(Self { account })
    }
}

impl std::ops::Deref for GasCallGuard<'_> {
    type Target = GasCounter;

    fn deref(&self) -> &Self::Target {
        self.account
    }
}

impl std::ops::DerefMut for GasCallGuard<'_> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.account
    }
}

impl Drop for GasCallGuard<'_> {
    fn drop(&mut self) {
        self.account.leave();
    }
}

impl Default for GasCounter {
    fn default() -> Self {
        Self {
            remaining: 0,
            purity_fee: 0,
            purity_size: 0,
            gas_rate: 1,
            initialized: false,
            reentry_depth: 0,
            max_reentry: 4,
        }
    }
}

impl GasCounter {

    /// Initialize gas budget from tx and chain parameters. Idempotent (only runs once).
    fn init_once(&mut self, ctx: &mut dyn Context, extra: &GasExtra, cap: &SpaceCap) -> Rerr {
        if self.initialized {
            if self.remaining <= 0 {
                return errf!("gas has run out")
            }
            return Ok(())
        }
        // cache tx ref to avoid repeated vtable dispatch on &dyn TransactionRead
        let tx = ctx.tx();
        // decode gas budget from tx.gas_max (1-byte lookup table)
        let gas_max_byte = tx.fee_extend()?;
        if gas_max_byte == 0 {
            return errf!("gas_max_byte is 0: contract call requires tx.gas_max > 0")
        }
        let decoded = decode_gas_budget(gas_max_byte);
        let budget = decoded.min(extra.max_gas_of_tx); // clamp to chain limit
        if budget <= 0 {
            return errf!(
                "gas budget invalid after clamp: gas_max_byte={} decoded={} chain_cap={}",
                gas_max_byte,
                decoded,
                extra.max_gas_of_tx
            )
        }
        // cache fee purity components for settlement (avoid repeated conversion)
        // gas_price = fee_purity = fee_got / tx_size (miner-received fee per byte)
        // keep numerator/denominator separate to avoid integer division precision loss
        let purity_fee = tx.fee_got().to_238_u64().unwrap_or(0) as i128;
        let purity_size = tx.size() as i128;
        if purity_fee <= 0 || purity_size <= 0 {
            return errf!("tx fee or size invalid for gas: purity_fee={} purity_size={}", purity_fee, purity_size)
        }
        // verify sender has enough balance for worst-case burn:
        // max_burn = budget * purity_fee / (purity_size * gas_rate), rounded up
        let gas_rate = extra.gas_rate.max(1) as i128;
        let max_burn = {
            let num = (budget as i128)
                .checked_mul(purity_fee)
                .ok_or_else(|| format!("max gas burn overflow: budget={} purity_fee={}", budget, purity_fee))?;
            let den = purity_size.checked_mul(gas_rate)
                .ok_or_else(|| format!("gas rate overflow: purity_size={} gas_rate={}", purity_size, gas_rate))?;
            (num + den - 1) / den // ceil division
        };
        if max_burn > u64::MAX as i128 {
            return errf!("max gas burn overflow: {}", max_burn)
        }
        let main = ctx.env().tx.main;
        let max_burn_amt = Amount::unit238(max_burn as u64);
        protocol::operate::hac_check(ctx, &main, &max_burn_amt)?;

        self.remaining = budget;
        self.purity_fee = purity_fee;
        self.purity_size = purity_size;
        self.gas_rate = extra.gas_rate.max(1);
        self.max_reentry = cap.max_reentry_depth;
        self.initialized = true;
        Ok(())
    }

    /// Enter a call layer. Increments depth, enforces re-entry limit.
    fn enter(&mut self) -> Rerr {
        let next_depth = self
            .reentry_depth
            .checked_add(1)
            .ok_or_else(|| "re-entry depth overflow".to_owned())?;
        if next_depth > self.max_reentry + 1 {
            // depth 1 = outermost call, depth 2 = first re-entry, etc.
            return errf!("re-entry depth {} exceeded limit {}", next_depth - 1, self.max_reentry)
        }
        self.reentry_depth = next_depth;
        Ok(())
    }

    /// Leave a call layer. Decrements depth.
    fn leave(&mut self) {
        self.reentry_depth = self.reentry_depth.saturating_sub(1);
    }

    /// Whether we are in the outermost VM call (depth == 1).
    /// Only the outermost call should settle (burn) HAC.
    fn is_outermost(&self) -> bool {
        self.reentry_depth == 1
    }

    /// Consume gas from the remaining budget.
    #[allow(dead_code)]
    fn consume(&mut self, amount: i64) -> Rerr {
        self.remaining -= amount;
        if self.remaining < 0 {
            return errf!("gas has run out")
        }
        Ok(())
    }

    /// Settle gas fee: burn HAC from sender's balance.
    /// Formula: burn_amt = ceil(cost * purity_fee / (purity_size * gas_rate))
    /// Single division — fee purity and settle share the same per-byte rate.
    fn settle(&self, ctx: &mut dyn Context, cost: i64) -> Rerr {
        if cost <= 0 {
            return errf!("gas cost invalid: {}", cost)
        }
        let gas_rate = self.gas_rate.max(1) as i128;
        let num = (cost as i128)
            .checked_mul(self.purity_fee)
            .ok_or_else(|| format!("gas burn overflow: cost={} purity_fee={}", cost, self.purity_fee))?;
        let den = self.purity_size
            .checked_mul(gas_rate)
            .ok_or_else(|| format!("gas rate overflow: purity_size={} rate={}", self.purity_size, gas_rate))?;
        if den <= 0 {
            return errf!("gas settle denominator invalid: purity_size={} rate={}", self.purity_size, gas_rate)
        }
        let burn = (num + den - 1) / den; // ceil division, at least 1
        if burn <= 0 {
            return errf!("gas burn underflow: cost={} purity_fee={} purity_size={} rate={}",
                cost, self.purity_fee, self.purity_size, gas_rate)
        }
        if burn > u64::MAX as i128 {
            return errf!("gas burn overflow: {}", burn)
        }
        let amt = Amount::unit238(burn as u64);
        let main = ctx.env().tx.main;
        protocol::operate::hac_sub(ctx, &main, &amt)?;
        Ok(())
    }
}


/*********************************/


#[allow(dead_code)]
pub struct MachineBox {
    account: GasCounter,
    machine: Option<Machine>,
} 

impl Drop for MachineBox {
    fn drop(&mut self) {
        match self.machine.take() {
            Some(m) => global_machine_manager().reclaim(m.r),
            _ => ()
        }
    }
}

impl MachineBox {
    
    pub fn new(m: Machine) -> Self {
        Self { 
            account: GasCounter::default(),
            machine: Some(m),
        }
    }
}

impl VM for MachineBox {
    fn usable(&self) -> bool { true }

    fn snapshot_volatile(&self) -> Box<dyn Any> {
        let m = self.machine.as_ref().unwrap();
        // IMPORTANT: Gas budget is deliberately excluded.
        // AstSelect/AstIf recover must rollback state/log/memory, but gas consumption stays monotonic.
        Box::new((
            m.r.global_vals.clone(),
            m.r.memory_vals.clone(),
            m.r.contracts.clone(),
            m.r.contract_load_bytes,
        ))
    }

    fn restore_volatile(&mut self, snap: Box<dyn Any>) {
        let Ok(snap) = snap.downcast::<(GKVMap, CtcKVMap, HashMap<ContractAddress, Arc<ContractObj>>, usize)>() else { return };
        let (globals, memorys, contracts, load_bytes) = *snap;
        let m = self.machine.as_mut().unwrap();
        m.r.global_vals = globals;
        m.r.memory_vals = memorys;
        m.r.contracts = contracts;
        m.r.contract_load_bytes = load_bytes;
    }

    fn call(&mut self, call: VMCall<'_>) -> Ret<(i64, Vec<u8>)> {
        use ExecMode::*;
        let VMCall {
            ctx,
            mode,
            kind,
            payload,
            param,
        } = call;
        // (1) initialize gas budget on first call (idempotent)
        {
            let r = &self.machine.as_ref().unwrap().r;
            self.account.init_once(ctx, &r.gas_extra, &r.space_cap)?;
        }
        // (2) enter call layer (depth check). Guard guarantees leave() on all exits.
        let mut account = GasCallGuard::enter(&mut self.account)?;
        let is_outermost = account.is_outermost();
        // min gas cost per call type
        let cty: ExecMode = std_mem_transmute!(mode);
        let min_cost = {
            let gsext = &self.machine.as_ref().unwrap().r.gas_extra;
            match cty {
                Main => gsext.main_call_min,
                P2sh => gsext.p2sh_call_min,
                Abst => gsext.abst_call_min,
                _ => never!(),
            }
        };
        // (3) execute VM call with shared gas counter
        let gas = &mut account.remaining;
        let gas_before = *gas;
        // Fail-fast: if remaining gas can't cover the per-call minimum, this call cannot start.
        if gas_before < min_cost {
            *gas -= min_cost; // keep the same "min cost consumes from shared counter" semantics
            return errf!(
                "gas budget too low: remaining={} < min_call_cost={} (mode={:?})",
                gas_before,
                min_cost,
                cty
            )
        }
        let machine = self.machine.as_mut().unwrap();
        let exenv = &mut ExecEnv{ ctx, gas };
        let result = match cty {
            Main => {
                let cty = CodeType::parse(kind)?;
                machine.main_call(exenv, cty, payload)
            }
            P2sh => {
                let payload = ByteView::from_arc(payload);
                let payload_ref = payload.as_slice();
                let (state_addr, mv1) = Address::create(payload_ref)?;
                let (calibs, mv2) = ContractAddressW1::create(&payload_ref[mv1..])?;
                let mv = mv1 + mv2;
                let realcodes = payload.slice(mv, payload.len())?;
                let Ok(param) = param.downcast::<Value>() else {
                    return errf!("p2sh argv type not match")
                };
                machine.p2sh_call(exenv, state_addr, calibs.into_list(), realcodes, *param)
            }
            Abst => {
                let kid: AbstCall = std_mem_transmute!(kind);
                let cadr = ContractAddress::parse(payload.as_ref())?;
                let Ok(param) = param.downcast::<Value>() else {
                    return errf!("abst argv type not match")
                };
                machine.abst_call(exenv, kid, cadr, *param)
            }
            _ => unreachable!()
        };
        // (4) compute gas cost, enforce minimum, leave call layer
        let gas = &mut account.remaining;
        let gas_after = *gas;
        let actual = gas_before - gas_after;
        let mut cost = actual;
        // enforce per-call minimum gas by consuming shortfall from shared counter
        if cost < min_cost {
            let shortfall = min_cost - cost;
            *gas -= shortfall;
            if *gas < 0 {
                return errf!(
                    "gas has run out after min cost enforcement: remaining={} (before={} min_call_cost={} actual_cost={})",
                    *gas,
                    gas_before,
                    min_cost,
                    actual
                );
            }
            cost = min_cost;
        }
        // propagate VM execution error (depth is auto-restored by guard drop)
        let resv = result.map(|a| a.raw())?;
        if cost <= 0 {
            return errf!("gas cost error: {}", cost);
        }
        // (5) settle: only the outermost call burns HAC
        if is_outermost {
            account.settle(ctx, cost)?;
        }
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

    pub fn main_call(&mut self, env: &mut ExecEnv, ctype: CodeType, codes: Arc<[u8]>) -> Ret<Value> {
        let fnobj = FnObj::plain(ctype, codes, 0, None);
        let ctx_adr = ContractAddress::from_unchecked(env.ctx.tx().main());
        let lib_adr = env.ctx.env().tx.addrs.iter().map(|a|ContractAddress::from_unchecked(*a)).collect();
        let rv = self.do_call(env, ExecMode::Main, &fnobj, ctx_adr, None, Some(lib_adr), None)?;
        check_vm_return_value(&rv, "main call")?;
        Ok(rv)
    }

    pub fn abst_call(&mut self, env: &mut ExecEnv, cty: AbstCall, contract_addr: ContractAddress, param: Value) -> Ret<Value> {
        let adr = contract_addr.to_readable();
        let Some((owner, fnobj)) = self.r.load_abstfn(env.ctx, &contract_addr, cty)? else {
            return errf!("abst call {:?} not find in {}", cty, adr)
        };
        let rv = self.do_call(env, ExecMode::Abst, fnobj.as_ref(), contract_addr, owner, None, Some(param))?;
        check_vm_return_value(&rv, &format!("call {}.{:?}", adr, cty))?;
        Ok(rv)
    }

    fn p2sh_call(&mut self, env: &mut ExecEnv, p2sh_addr: Address, libs: Vec<ContractAddress>, codes: ByteView, param: Value) -> Ret<Value> {
        let ctype = CodeType::Bytecode;
        let fnobj = FnObj::plain(ctype, codes, 0, None);
        let ctx_adr = ContractAddress::from_unchecked(p2sh_addr);
        let rv = self.do_call(env, ExecMode::P2sh, &fnobj, ctx_adr, None, Some(libs), Some(param))?;
        check_vm_return_value(&rv, "p2sh call")?;
        Ok(rv)
    }

    fn do_call(&mut self, env: &mut ExecEnv, mode: ExecMode, code: &FnObj, entry_addr: ContractAddress, code_owner: Option<ContractAddress>, libs: Option<Vec<ContractAddress>>, param: Option<Value>) -> VmrtRes<Value> {
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
    use basis::interface::Context;
    use field::{Address, Amount, Uint4};
    use testkit::sim::context::make_ctx_with_state;
    use testkit::sim::state::FlatMemState as StateMem;
    use testkit::sim::tx::StubTxBuilder;

    #[test]
    fn calltargets_resolve_under_callview_and_inherits() {
        // Arrange addresses.
        let base_addr = Address::from_readable("1MzNY1oA3kfgYi75zquj3SRUPYztzXHzK9").unwrap();
        let contract_child = crate::ContractAddress::calculate(&base_addr, &Uint4::from(1));
        let contract_parent = crate::ContractAddress::calculate(&base_addr, &Uint4::from(2));
        let contract_base = crate::ContractAddress::calculate(&base_addr, &Uint4::from(3));

        // Build an inheritance chain: Child -> Parent -> Base.
        // The key trick is: `super.f()` moves code_owner to Parent, while state_addr stays Child.
        // Then inside Parent.f(), `this.g()` must resolve in state_addr (Child), `self.g()` in code_owner (Parent),
        // and `super.g()` in Parent's direct base (Base).

        let base = Contract::new().func(Func::new("g").unwrap().fitsh("return 3").unwrap());

        let parent = Contract::new()
            .inh(contract_base.to_addr())
            .func(Func::new("g").unwrap().fitsh("return 2").unwrap())
            .func(
                Func::new("f").unwrap()
                    .fitsh(
                        r##"
                        return this.g() * 10000 + self.g() * 100 + super.g()
                        "##,
                    )
                    .unwrap(),
            );

        let child = Contract::new()
            .inh(contract_parent.to_addr())
            .func(Func::new("g").unwrap().fitsh("return 1").unwrap())
            .func(
                Func::new("run").unwrap()
                    .public()
                    .fitsh(
                        r##"
                        let v = super.f()
                        assert v == 10203
                        return 0
                        "##,
                    )
                    .unwrap(),
            );

        // Put contracts into state, then move it into Context.
        let mut ext_state = StateMem::default();
        {
            let mut vmsta = crate::VMState::wrap(&mut ext_state);
            vmsta.contract_set(&contract_base, &base.into_sto());
            vmsta.contract_set(&contract_parent, &parent.into_sto());
            vmsta.contract_set(&contract_child, &child.into_sto());
        }

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

        let tx = StubTxBuilder::new()
            .ty(0)
            .main(base_addr)
            .addrs(env.tx.addrs.clone())
            .fee(Amount::zero())
            .gas_max(1)
            .tx_size(128)
            .fee_purity(1)
            .build();
        let mut ctx = make_ctx_with_state(env, Box::new(ext_state), &tx);

        let mut gas: i64 = 1_000_000;
        let mut exec = crate::frame::ExecEnv {
            ctx: &mut ctx as &mut dyn Context,
            gas: &mut gas,
        };

        let mut machine = Machine::create(Resoure::create(1));
        let rv = machine
            .main_call(&mut exec, CodeType::Bytecode, main_codes.into())
            .unwrap();

        assert!(!rv.check_true(), "main call should return success (nil/0)");
    }

    #[test]
    fn call_outer_uses_inherits_but_view_pure_callcode_keep_local_lookup() {
        let base_addr = Address::from_readable("1MzNY1oA3kfgYi75zquj3SRUPYztzXHzK9").unwrap();
        let contract_child = crate::ContractAddress::calculate(&base_addr, &Uint4::from(11));
        let contract_parent = crate::ContractAddress::calculate(&base_addr, &Uint4::from(12));

        let parent_sto = Contract::new()
            .func(Func::new("id").unwrap().fitsh("return 2").unwrap())
            .func(
                Func::new("probe")
                    .unwrap()
                    .public()
                    .fitsh("return self.id() * 100 + this.id()")
                    .unwrap(),
            )
            .into_sto();
        let child_sto = Contract::new()
            .inh(contract_parent.to_addr())
            .func(Func::new("id").unwrap().fitsh("return 1").unwrap())
            .func(Func::new("noop").unwrap().public().fitsh("return 0").unwrap())
            .into_sto();

        let run_main = |main_script: &str| -> Ret<Value> {
            let main_codes = lang_to_bytecode(main_script).unwrap();
            let mut ext_state = StateMem::default();
            {
                let mut vmsta = crate::VMState::wrap(&mut ext_state);
                vmsta.contract_set(&contract_parent, &parent_sto.clone());
                vmsta.contract_set(&contract_child, &child_sto.clone());
            }

            let mut env = Env::default();
            env.block.height = 1;
            env.tx.main = base_addr;
            env.tx.addrs = vec![contract_child.clone().into_addr()];

            let tx = StubTxBuilder::new()
                .ty(0)
                .main(base_addr)
                .addrs(env.tx.addrs.clone())
                .fee(Amount::zero())
                .gas_max(1)
                .tx_size(128)
                .fee_purity(1)
                .build();
            let mut ctx = make_ctx_with_state(env, Box::new(ext_state), &tx);

            let mut gas: i64 = 1_000_000;
            let mut exec = crate::frame::ExecEnv {
                ctx: &mut ctx as &mut dyn Context,
                gas: &mut gas,
            };
            let mut machine = Machine::create(Resoure::create(1));
            machine.main_call(&mut exec, CodeType::Bytecode, main_codes.into())
        };

        // CALL (Outer): should resolve inherited `probe` on parent.
        let outer_script = r##"
            lib C = 0
            let v = C.probe()
            // In parent.probe():
            // - self.id() resolves in parent(code_owner)=2
            // - this.id() resolves in child(state_addr)=1
            assert v == 201
            return 0
        "##;
        assert!(run_main(outer_script).is_ok(), "CALL should resolve through inherits");

        // CALLVIEW/CALLPURE/CALLCODE: should keep exact local lookup, so inherited-only fn is not found.
        let view_script = r##"
            lib C = 0
            return C:probe()
        "##;
        assert!(run_main(view_script).is_err(), "CALLVIEW should not resolve inherits");

        let pure_script = r##"
            lib C = 0
            return C::probe()
        "##;
        assert!(run_main(pure_script).is_err(), "CALLPURE should not resolve inherits");

        let callcode_script = r##"
            lib C = 0
            callcode C::probe
            end
        "##;
        assert!(run_main(callcode_script).is_err(), "CALLCODE should not resolve inherits");
    }

    #[test]
    fn min_call_gas_is_consumed_from_shared_counter() {
        use crate::rt::Bytecode;

        // Prepare a state with enough HAC for gas spending.
        let main = Address::from_readable("1MzNY1oA3kfgYi75zquj3SRUPYztzXHzK9").unwrap();
        let mut env = Env::default();
        env.block.height = 1;
        env.tx.main = main;
        env.tx.addrs = vec![main];

        let tx = StubTxBuilder::new()
            .ty(TransactionType3::TYPE)
            .main(main)
            .addrs(vec![main])
            .fee(Amount::unit238(10_000_000))
            .gas_max(17)
            .tx_size(128)
            .fee_purity(3200)
            .build();
        let mut ctx = make_ctx_with_state(env, Box::new(StateMem::default()), &tx);
        protocol::operate::hac_add(&mut ctx, &main, &Amount::unit238(1_000_000_000)).unwrap();

        let mut vm = MachineBox::new(Machine::create(Resoure::create(1)));
        // END is a minimal "return nil" program; actual instruction gas is tiny.
        let codes = vec![Bytecode::END as u8];

        vm.call(VMCall::new(&mut ctx, ExecMode::Main as u8, CodeType::Bytecode as u8, codes.clone().into(), Box::new(Value::Nil)))
            .unwrap();

        let gsext = GasExtra::new(1);
        let min = gsext.main_call_min;
        let budget = decode_gas_budget(17); // lookup-table decoded budget
        // The min-call cost must be reflected in the shared gas counter.
        assert!(vm.account.remaining <= (budget - min), "account.remaining should include min cost deduction");
    }

    #[test]
    fn snapshot_restore_volatile_fields_only_except_gas_remaining() {
        use crate::rt::Bytecode;

        let main = Address::from_readable("1MzNY1oA3kfgYi75zquj3SRUPYztzXHzK9").unwrap();
        let mut env = Env::default();
        env.block.height = 1;
        env.tx.main = main;
        env.tx.addrs = vec![main];

        let tx = StubTxBuilder::new()
            .ty(TransactionType3::TYPE)
            .main(main)
            .addrs(vec![main])
            .fee(Amount::unit238(10_000_000))
            .gas_max(17)
            .tx_size(128)
            .fee_purity(1)
            .build();
        let mut ctx = make_ctx_with_state(env, Box::new(StateMem::default()), &tx);
        protocol::operate::hac_add(&mut ctx, &main, &Amount::unit238(1_000_000_000)).unwrap();

        let mut vm = MachineBox::new(Machine::create(Resoure::create(1)));
        let codes = vec![Bytecode::END as u8];
        vm.call(VMCall::new(&mut ctx, ExecMode::Main as u8, CodeType::Bytecode as u8, codes.clone().into(), Box::new(Value::Nil))).unwrap();

        vm.machine.as_mut().unwrap().r.contract_load_bytes = 11;
        let before_load_bytes = vm.machine.as_ref().unwrap().r.contract_load_bytes;
        let snap = vm.snapshot_volatile();

        // Mutate fields that are now outside VM volatile snapshot.
        vm.account.remaining = 1;
        // Mutate volatile fields (should be restored)
        vm.machine.as_mut().unwrap().r.contract_load_bytes = 777;

        // Mutate non-volatile fields (should NOT be restored — init_once/RAII managed)
        vm.account.purity_fee = 1;
        vm.account.purity_size = 1;
        vm.account.gas_rate = 99;
        vm.account.initialized = false;
        vm.account.reentry_depth = 3;
        vm.account.max_reentry = 99;

        vm.restore_volatile(snap);

        // Gas remaining is NOT restored: gas usage must stay monotonic in one tx.
        assert_eq!(vm.account.remaining, 1);
        // Volatile fields: restored to snapshot values
        assert_eq!(vm.machine.as_ref().unwrap().r.contract_load_bytes, before_load_bytes);

        // Non-volatile fields: NOT restored (keep mutated values)
        assert_eq!(vm.account.purity_fee, 1);
        assert_eq!(vm.account.purity_size, 1);
        assert_eq!(vm.account.gas_rate, 99);
        assert_eq!(vm.account.initialized, false);
        assert_eq!(vm.account.reentry_depth, 3);
        assert_eq!(vm.account.max_reentry, 99);
    }

    #[test]
    fn low_fee_does_not_panic_in_settle() {
        use crate::rt::Bytecode;

        let main = Address::from_readable("1MzNY1oA3kfgYi75zquj3SRUPYztzXHzK9").unwrap();
        let mut env = Env::default();
        env.block.height = 1;
        env.tx.main = main;
        env.tx.addrs = vec![main];

        let tx = StubTxBuilder::new()
            .ty(TransactionType3::TYPE)
            .main(main)
            .addrs(vec![main])
            .fee(Amount::unit238(1))
            .gas_max(1)
            .tx_size(128)
            .fee_purity(1)
            .build();
        let mut ctx = make_ctx_with_state(env, Box::new(StateMem::default()), &tx);
        protocol::operate::hac_add(&mut ctx, &main, &Amount::unit238(1_000_000_000)).unwrap();

        let mut vm = MachineBox::new(Machine::create(Resoure::create(1)));
        let codes = vec![Bytecode::END as u8];

        let res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            vm.call(VMCall::new(&mut ctx, ExecMode::Main as u8, CodeType::Bytecode as u8, codes.clone().into(), Box::new(Value::Nil)))
        }));
        assert!(res.is_ok(), "settle must not panic");
        // low fee may cause an error return, but must never panic
        let _ = res.unwrap();
    }

    #[test]
    fn reentry_depth_restores_after_early_return() {
        use crate::rt::Bytecode;

        let main = Address::from_readable("1MzNY1oA3kfgYi75zquj3SRUPYztzXHzK9").unwrap();
        let mut env = Env::default();
        env.block.height = 1;
        env.tx.main = main;
        env.tx.addrs = vec![main];

        let tx = StubTxBuilder::new()
            .ty(TransactionType3::TYPE)
            .main(main)
            .addrs(vec![main])
            .fee(Amount::unit238(10_000_000))
            .gas_max(17)
            .tx_size(128)
            .fee_purity(1)
            .build();
        let mut ctx = make_ctx_with_state(env, Box::new(StateMem::default()), &tx);
        protocol::operate::hac_add(&mut ctx, &main, &Amount::unit238(1_000_000_000)).unwrap();

        let mut vm = MachineBox::new(Machine::create(Resoure::create(1)));
        // Invalid code type causes early return inside Main branch before previous manual leave() point.
        let early = vm.call(VMCall::new(
            &mut ctx,
            ExecMode::Main as u8,
            255,
            Arc::from(vec![]),
            Box::new(Value::Nil),
        ));
        assert!(early.is_err(), "invalid code type must fail");
        assert_eq!(vm.account.reentry_depth, 0, "depth must be restored after early return");

        // Next normal call should still behave as an outermost call.
        let codes = vec![Bytecode::END as u8];
        let ok = vm.call(VMCall::new(
            &mut ctx,
            ExecMode::Main as u8,
            CodeType::Bytecode as u8,
            codes.into(),
            Box::new(Value::Nil),
        ));
        assert!(ok.is_ok(), "subsequent call must not be poisoned by previous early return");
        assert_eq!(vm.account.reentry_depth, 0, "depth must remain balanced after successful call");
    }

/*
    i64::MAX  = 9223372036854775807
    10000 HAC =   10000000000000000:236

    0.00000001 = 1:240 = 10000:236




*/


}
