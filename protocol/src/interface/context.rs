
pub trait ExtActCal {
    fn height(&self) -> u64 { never!() } // ctx blk hei
    fn action_call(&mut self, _: u16, _: Vec<u8>) -> Ret<(u32, Vec<u8>)> { never!() }
}

pub trait Context : ExtActCal {
    fn as_ext_caller(&mut self) -> &mut dyn ExtActCal { never!() }
    fn env(&self) -> &Env { never!() }
    fn addr(&self, _:&AddrOrPtr) -> Ret<Address> { never!() }
    fn state(&mut self) -> &mut dyn State { never!() }
    fn state_fork(&mut self) -> Box<dyn State> { never!() }
    fn state_merge(&mut self, _: Box<dyn State>) { never!() }
    fn state_replace(&mut self, _: Box<dyn State>) -> Box<dyn State> { never!() }
    fn check_sign(&mut self, _: &Address) -> Rerr { never!() }
    fn depth(&mut self) -> &mut CallDepth { never!() }
    fn depth_set(&mut self, _: CallDepth) { never!() }
    // fn depth_add(&mut self) { never!() }
    // fn depth_sub(&mut self) { never!() }
    fn tx(&self) -> &dyn TransactionRead { never!() }
    fn vm(&mut self) -> &mut dyn VMI { never!() }
    fn vm_replace(&mut self, _: Box<dyn VMI>) -> Box<dyn VMI> { never!() }
    
}

