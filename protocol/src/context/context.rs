use crate::CallDepth;


/*
*/
pub struct ContextInst<'a> {
    pub env: Env,
    pub depth: CallDepth,
    pub txr: &'a dyn TransactionRead,

    pub vmi: Box<dyn VMI>,

    sta: Box<dyn State>,
    check_sign_cache: HashMap<Address, Ret<bool>>,
}


impl ContextInst<'_> {

    pub fn new<'a>(env: Env, sta: Box<dyn State>, txr: &'a dyn TransactionRead) -> ContextInst<'a> {
        ContextInst{ env, sta, txr,
            depth: CallDepth::new(0),
            check_sign_cache: HashMap::new(),
            vmi: VMNil::empty(),
        }
    }

    pub fn into_state(self) -> Box<dyn State> {
        self.sta
    }
}

impl ExtActCal for ContextInst<'_> {
    fn height(&self) -> u64 { self.env.block.height }
    fn action_call(&mut self, k: u16, b: Vec<u8>) -> Ret<(u32, Vec<u8>)> {
        ctx_action_call(self, k, b)
    }
}

impl Context for ContextInst<'_> {
    fn as_ext_caller(&mut self) -> &mut dyn ExtActCal { self }
    // fn env(&self) -> &Env { &self.env}
    fn state(&mut self) -> &mut dyn State { self.sta.as_mut() }
    fn state_fork(&mut self) -> Box<dyn State> {
        ctx_state_fork_sub(self)
    }
    fn state_merge(&mut self, old: Box<dyn State>){
        ctx_state_merge_sub(self, old)
    }
    fn state_replace(&mut self, sta: Box<dyn State>) -> Box<dyn State> {
        std::mem::replace(&mut self.sta, sta)
    }
    fn depth(&mut self) -> &mut CallDepth { &mut self.depth }
    /*
    fn depth_set(&mut self, d: i8) { self.depth = d }
    fn depth_add(&mut self) { self.depth += 1 }
    fn depth_sub(&mut self) { self.depth -= 1 }
    */

    fn tx(&self) -> &dyn TransactionRead { self.txr }
    fn vm(&mut self) -> &mut dyn VMI { self.vmi.as_mut() }
    fn vm_replace(&mut self, vm: Box<dyn VMI>) -> Box<dyn VMI> {
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
}

