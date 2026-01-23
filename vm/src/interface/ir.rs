use std::cell::RefCell;
use std::collections::HashSet;
use std::rc::Rc;

use crate::rt::{Bytecode, SourceMap};

#[derive(Clone)]
pub struct PrintOption<'a> {
    pub indent: &'a str,
    pub tab: usize,
    pub desc: bool,
    pub map: Option<&'a SourceMap>,
    pub trim_root_block: bool,
    pub trim_head_alloc: bool,
    pub trim_param_unpack: bool,
    pub hide_func_nil_argv: bool,
    allocated: Rc<RefCell<HashSet<u8>>>,
}

impl<'a> PrintOption<'a> {
    pub fn new(indent: &'a str, tab: usize, desc: bool) -> Self {
        Self {
            indent,
            tab,
            desc,
            map: None,
            trim_root_block: false,
            trim_head_alloc: false,
            trim_param_unpack: false,
            hide_func_nil_argv: false,
            allocated: Rc::new(RefCell::new(HashSet::new())),
        }
    }

    pub fn with_trim_root_block(mut self, trim: bool) -> Self {
        self.trim_root_block = trim;
        self
    }

    pub fn with_trim_head_alloc(mut self, trim: bool) -> Self {
        self.trim_head_alloc = trim;
        self
    }

    pub fn with_source_map(mut self, map: &'a SourceMap) -> Self {
        self.map = Some(map);
        self.allocated = map.slot_puts();
        self
    }

    pub fn with_trim_param_unpack(mut self, trim: bool) -> Self {
        self.trim_param_unpack = trim;
        self
    }

    pub fn with_hide_func_nil_argv(mut self, hide: bool) -> Self {
        self.hide_func_nil_argv = hide;
        self
    }

    pub fn with_tab(&self, tab: usize) -> Self {
        let mut next = self.clone();
        next.tab = tab;
        next
    }

    pub fn child(&self) -> Self {
        self.with_tab(self.tab + 1)
    }

    pub fn mark_slot_put(&self, slot: u8) -> bool {
        self.allocated.borrow_mut().insert(slot)
    }

    pub fn clear_slot_put(&self, slot: u8) {
        self.allocated.borrow_mut().remove(&slot);
    }

    pub fn clear_all_slot_puts(&self) {
        self.allocated.borrow_mut().clear();
    }
}

pub trait IRNode : DynClone {
    fn bytecode(&self) -> u8;
    fn hasretval(&self) -> bool;
    fn subs(&self) -> usize { 0 }
    fn level(&self) -> u8 { 0 }
    fn checkretval(&self) -> Rerr { 
        match self.hasretval() {
            true => Ok(()),
            false => {
                let c: Bytecode = std_mem_transmute!( self.bytecode() );
                let n = c.metadata().intro;
                errf!("ir build error: Inst {:?} ({}) not have return value", c, n)
            }
        }
    }
    fn print(&self, _opt: &PrintOption) -> String {
        "IRNode".to_owned()
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
