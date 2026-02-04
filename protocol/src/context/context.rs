
/*
*/
pub struct ContextInst<'a> {
    pub env: Env,
    pub depth: CallDepth,
    pub txr: &'a dyn TransactionRead,

    pub vmi: Box<dyn VM>,

    pub tex_ledger: TexLedger,

    log: Box<dyn Logs>,
    sta: Box<dyn State>,
    psh: HashMap<Address, Box<dyn P2sh>>,

    check_sign_cache: HashMap<Address, Ret<bool>>,
}


impl ContextInst<'_> {

    pub fn new<'a>(env: Env, sta: Box<dyn State>, log: Box<dyn Logs>, txr: &'a dyn TransactionRead) -> ContextInst<'a> {
        ContextInst{ env, sta, log, txr,
            depth: CallDepth::new(0),
            check_sign_cache: HashMap::new(),
            vmi: VMNil::empty(),
            psh: HashMap::default(), 
            tex_ledger: TexLedger::default(),
        }
    }

    pub fn release(self) -> (Box<dyn State>, Box<dyn Logs>) {
        (self.sta, self.log)
    }
}

impl ActCall for ContextInst<'_> {
    fn height(&self) -> u64 { self.env.block.height }
    fn action_call(&mut self, k: u16, b: Vec<u8>) -> Ret<(u32, Vec<u8>)> {
        ctx_action_call(self, k, b)
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
        ctx_state_recover(self, old)
    }
    fn state_replace(&mut self, sta: Box<dyn State>) -> Box<dyn State> {
        std::mem::replace(&mut self.sta, sta)
    }
}

impl Context for ContextInst<'_> {

    fn logs(&mut self) -> &mut dyn Logs {
        self.log.as_mut()
    }

    fn reset_for_new_tx(&mut self) {
        // Per-tx caches must not leak across transactions.
        self.psh.clear();
        self.check_sign_cache.clear();
        self.tex_ledger = TexLedger::default();
        self.vm_replace(VMNil::empty());
    }
    fn as_ext_caller(&mut self) -> &mut dyn ActCall { self }
    fn env(&self) -> &Env { &self.env }
    
    fn depth(&mut self) -> &mut CallDepth { &mut self.depth }
    fn depth_set(&mut self, cd: CallDepth) { self.depth = cd }
    /*
    fn depth_add(&mut self) { self.depth += 1 }
    fn depth_sub(&mut self) { self.depth -= 1 }
    */

    fn tx(&self) -> &dyn TransactionRead { self.txr }
    fn vm(&mut self) -> &mut dyn VM { self.vmi.as_mut() }
    fn vm_replace(&mut self, vm: Box<dyn VM>) -> Box<dyn VM> {
        std::mem::replace(&mut self.vmi, vm)
    }
    fn addr(&self, ptr :&AddrOrPtr) -> Ret<Address> {
        ptr.real(&self.env.tx.addrs)
    }
    fn check_sign(&mut self, adr: &Address) -> Rerr {
        adr.must_privakey()?;
        if self.check_sign_cache.contains_key(adr) {
            return self.check_sign_cache[adr].clone().map(|_|())
        }
        let isok = transaction::verify_target_signature(adr, self.txr);
        self.check_sign_cache.insert(*adr, isok.clone());
        isok.map(|_|())
    }
    // tex
    fn tex_ledger(&mut self) -> &mut TexLedger {
        &mut self.tex_ledger
    }
    // p2sh
    fn p2sh(&self, adr: &Address) -> Ret<&Box<dyn P2sh>> {
        let e = format!("p2sh '{}' not find", adr.readable());  
        self.psh.get(adr).ok_or(e)
    }
    fn p2sh_set(&mut self, adr: Address, p2sh: Box<dyn P2sh>) -> Rerr {
        adr.must_scriptmh()?;
        // Avoid ambiguity: a tx should not be able to "re-prove" the same scriptmh address with
        // different code. Keeping a single, unique proof per address makes tx validation and
        // mempool simulation deterministic and easier to audit.
        if self.psh.contains_key(&adr) {
            return errf!("p2sh '{}' already proved in current tx", adr.readable())
        }
        self.psh.insert(adr, p2sh);
        Ok(())
    }
    // psh: HashMap<Address, Box<dyn P2sh>>,

}
