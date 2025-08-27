
pub trait ExtActCal {
    fn height(&self) -> u64; // ctx blk hei
    fn action_call(&mut self, _: u16, _: Vec<u8>) -> Ret<(u32, Vec<u8>)>;
}

pub trait Context : ExtActCal {
    fn as_ext_caller(&mut self) -> &mut dyn ExtActCal;
    fn env(&self) -> &Env;
    fn addr(&self, _:&AddrOrPtr) -> Ret<Address>;
    fn state(&mut self) -> &mut dyn State;
    fn state_fork(&mut self) -> Box<dyn State>;
    fn state_merge(&mut self, _: Box<dyn State>);
    fn state_replace(&mut self, _: Box<dyn State>) -> Box<dyn State>;
    fn check_sign(&mut self, _: &Address) -> Rerr;
    fn depth(&mut self) -> &mut CallDepth;
    fn depth_set(&mut self, _: CallDepth);
    // fn depth_add(&mut self) { never!() }
    // fn depth_sub(&mut self) { never!() }
    fn tx(&self) -> &dyn TransactionRead;
    fn vm(&mut self) -> &mut dyn VMI;
    fn vm_replace(&mut self, _: Box<dyn VMI>) -> Box<dyn VMI>;
    
}

