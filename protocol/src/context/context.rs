
/*
*/
pub struct ContextInst<'a> {
    pub env: Env,
    pub level: usize,
    pub exec_from: ActExecFrom,
    pub txr: &'a dyn TransactionRead,

    pub vmi: Box<dyn VM>,

    pub tex_ledger: TexLedger,
    gas: GasCounter,

    log: Box<dyn Logs>,
    sta: Box<dyn State>,
    psh: HashMap<Address, Box<dyn P2sh>>,

    check_sign_cache: HashMap<Address, Ret<bool>>,
}


impl ContextInst<'_> {

    pub fn new<'a>(env: Env, sta: Box<dyn State>, log: Box<dyn Logs>, txr: &'a dyn TransactionRead) -> ContextInst<'a> {
        ContextInst{ env, sta, log, txr,
            level: ACTION_CTX_LEVEL_TOP,
            exec_from: ActExecFrom::TxLoop,
            check_sign_cache: HashMap::new(),
            vmi: VMNil::empty(),
            psh: HashMap::new(),
            tex_ledger: TexLedger::default(),
            gas: GasCounter::default(),
        }
    }

    #[inline]
    fn tx_ref(&self) -> &dyn TransactionRead {
        self.txr
    }

    #[inline]
    fn tx_addrs(&self) -> &Vec<Address> {
        &self.env.tx.addrs
    }

    #[inline]
    fn debug_assert_tx_bound_consistent(&self) {
        #[cfg(debug_assertions)]
        {
            debug_assert_eq!(self.env.tx.ty, self.tx_ref().ty());
            debug_assert_eq!(self.env.tx.main, self.tx_ref().main());
            debug_assert_eq!(self.env.tx.addrs, self.tx_ref().addrs());
        }
    }

    #[inline]
    fn reset_tx_runtime_state(&mut self) {
        // Per-tx caches must not leak across transactions.
        self.psh.clear();
        self.check_sign_cache.clear();
        self.tex_ledger = TexLedger::default();
        self.vmi = VMNil::empty();
        self.ctx_gas_reset();
        self.level = ACTION_CTX_LEVEL_TOP;
        self.exec_from = ActExecFrom::TxLoop;
    }

    #[inline]
    fn bind_tx(&mut self, txr: &dyn TransactionRead) {
        self.txr = unsafe { std::mem::transmute::<&dyn TransactionRead, &'static dyn TransactionRead>(txr) };
        self.env.replace_tx(create_tx_info(txr));
        self.debug_assert_tx_bound_consistent();
    }

    #[inline]
    fn state_replace(&mut self, sta: Box<dyn State>) -> Box<dyn State> {
        std::mem::replace(&mut self.sta, sta)
    }

    pub fn release(self) -> (Box<dyn State>, Box<dyn Logs>) {
        (self.sta, self.log)
    }
}

impl StateOperat for ContextInst<'_> {

    fn state(&mut self) -> &mut dyn State { self.sta.as_mut() }
    fn state_fork(&mut self) -> Arc<Box<dyn State>> {
        ctx_state_fork_sub(self)
    }
    fn state_merge(&mut self, old: Arc<Box<dyn State>>){
        ctx_state_merge_sub(self, old)
    }
    fn state_recover(&mut self, old: Arc<Box<dyn State>>) {
        ctx_state_recover_sub(self, old)
    }
}

impl Context for ContextInst<'_> {

    fn action_call(&mut self, k: u16, b: Vec<u8>) -> XRet<(u32, Vec<u8>)> {
        ctx_action_call(self, k, b)
    }
    fn action_exec_from(&self) -> ActExecFrom { self.exec_from }
    fn action_exec_from_set(&mut self, src: ActExecFrom) { self.exec_from = src }

    fn logs(&mut self) -> &mut dyn Logs {
        self.log.as_mut()
    }

    fn snapshot_volatile(&self) -> Box<dyn Any> {
        // Note: `level` is NOT included here because AstLevelGuard (RAII)
        // already restores it on all exit paths (success, error, panic).
        Box::new((
            self.tex_ledger.clone(),
            self.psh.keys().cloned().collect::<HashSet<Address>>(),
        ))
    }

    fn restore_volatile(&mut self, snap: Box<dyn Any>) {
        let Ok(snap) = snap.downcast::<(TexLedger, HashSet<Address>)>() else { return };
        let (tex, keys) = *snap;
        self.tex_ledger = tex;
        self.psh.retain(|k, _| keys.contains(k));
    }

    fn reset_for_new_tx(&mut self, txr: &dyn TransactionRead) {
        self.bind_tx(txr);
        self.reset_tx_runtime_state();
    }
    fn env(&self) -> &Env { &self.env }
    
    fn level(&self) -> usize { self.level }
    fn level_set(&mut self, level: usize) { self.level = level }

    fn tx(&self) -> &dyn TransactionRead { self.tx_ref() }
    fn vm(&mut self) -> &mut dyn VM { self.vmi.as_mut() }
    fn vm_init_once(&mut self, vm: Box<dyn VM>) -> Rerr {
        if !self.vmi.is_nil() {
            return errf!("vm already initialized")
        }
        self.vmi = vm;
        Ok(())
    }
    fn gas_init_tx(&mut self, budget: i64, gas_rate: i64) -> Rerr {
        self.ctx_gas_init_tx(budget, gas_rate)
    }
    fn gas_refund(&mut self) -> Rerr {
        self.ctx_gas_refund()
    }
    fn gas_remaining(&self) -> i64 {
        self.ctx_gas_remaining()
    }
    fn gas_consume(&mut self, gas: u32) -> Rerr {
        self.ctx_gas_consume(gas)
    }
    fn vm_gas_mut(&mut self) -> Ret<&mut dyn VmGasMut> {
        Ok(self)
    }
    fn addr(&self, ptr :&AddrOrPtr) -> Ret<Address> {
        self.debug_assert_tx_bound_consistent();
        ptr.real(self.tx_addrs())
    }
    fn check_sign(&mut self, adr: &Address) -> Rerr {
        self.debug_assert_tx_bound_consistent();
        if let Some(isok) = self.check_sign_cache.get(adr) {
            return isok.clone().map(|_|())
        }
        // Must check privkey after cache lookup to avoid unnecessary checks on cached entries
        adr.must_privakey()?;
        let isok = verify_target_signature(adr, self.tx_ref());
        self.check_sign_cache.insert(*adr, isok.clone());
        isok.map(|_|())
    }
    // tex
    fn tex_ledger(&mut self) -> &mut TexLedger {
        &mut self.tex_ledger
    }
    // p2sh
    fn p2sh(&self, adr: &Address) -> Ret<&dyn P2sh> {
        let e = format!("p2sh '{}' not found", adr);
        self.psh.get(adr).map(|boxed| boxed.as_ref()).ok_or(e)
    }
    fn p2sh_set(&mut self, adr: Address, p2sh: Box<dyn P2sh>) -> Rerr {
        adr.must_scriptmh()?;
        // Avoid ambiguity: a tx should not be able to "re-prove" the same scriptmh address with
        // different code. Keeping a single, unique proof per address makes tx validation and
        // mempool simulation deterministic and easier to audit.
        if self.psh.contains_key(&adr) {
            return errf!("p2sh '{}' already proved in current tx", adr)
        }
        self.psh.insert(adr, p2sh);
        Ok(())
    }
    // psh: HashMap<Address, Box<dyn P2sh>>,

}

impl VmGasMut for ContextInst<'_> {
    fn gas_remaining_mut(&mut self) -> Ret<&mut i64> {
        self.ctx_gas_remaining_mut()
    }
}
