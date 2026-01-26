use std::any::Any;

use dyn_clone::{clone_trait_object, DynClone};

use crate::rt::Bytecode;
use sys::Rerr;

pub trait IRNode : DynClone {
    fn bytecode(&self) -> u8;
    fn hasretval(&self) -> bool;
    fn subs(&self) -> usize { 0 }
    fn level(&self) -> u8 { 0 }
    fn checkretval(&self) -> Rerr {
        maybe!(self.hasretval(), Ok(()), {
            let c: Bytecode = std_mem_transmute!( self.bytecode() );
            let n = c.metadata().intro;
            errf!("ir build error: Inst {:?} ({}) not have return value", c, n)
        })
    }
    fn print(&self) -> String {
        let c: Bytecode = std_mem_transmute!(self.bytecode());
        format!("{:?}", c)
    }
    // fn childx(&self) -> &dyn IRNode { panic_never_call_this!() }
    // fn childy(&self) -> &dyn IRNode { panic_never_call_this!() }
    // fn childz(&self) -> &dyn IRNode { panic_never_call_this!() }
    // fn subnodes(&self) -> Vec<&dyn IRNode> { panic_never_call_this!() }
    // compile
    // fn parsing(&mut self, seek: &mut usize) -> RetErr { panic_never_call_this!() }
    fn codegen(&self) -> crate::VmrtRes<Vec<u8>> {
        let mut buf = Vec::new();
        self.codegen_into(&mut buf)?;
        Ok(buf)
    }
    fn codegen_into(&self, buf: &mut Vec<u8>) -> crate::VmrtRes<()> {
        buf.push(self.bytecode());
        Ok(())
    }
    fn serialize(&self) -> Vec<u8> { vec![] }
    /*{
        let (_, _, _, out) = Bytecode::metadata(std_mem_transmute!(self.bytecode()));
        out == 1
    }*/
    fn as_any(&self) -> &dyn Any { unimplemented!() }
    fn as_any_mut(&mut self) -> &mut dyn Any { unimplemented!() }
}


clone_trait_object!(IRNode);


impl std::fmt::Debug for dyn IRNode {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{{IRNode}}")
    }
}
