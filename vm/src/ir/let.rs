use std::cell::RefCell;
use std::rc::Rc;

use super::rt::Bytecode;

pub struct LetInfo {
    pub(crate) expr: Rc<Box<dyn IRNode>>,
    pub(crate) slot: Option<u8>,
    pub(crate) refs: u8,
    pub(crate) needs_slot: bool,
}

impl LetInfo {
    pub fn new(node: Box<dyn IRNode>) -> Self {
        Self {
            expr: Rc::new(node),
            slot: None,
            refs: 0,
            needs_slot: false,
        }
    }
}


fn dup_inst_node() -> Box<dyn IRNode> {
    Box::new(IRNodeLeaf::notext(true, Bytecode::DUP))
}

fn local_get_node(idx: u8) -> Box<dyn IRNode> {
    match idx {
        0 => Box::new(IRNodeLeaf { hrtv: true, inst: Bytecode::GET0, text: String::new() }),
        1 => Box::new(IRNodeLeaf { hrtv: true, inst: Bytecode::GET1, text: String::new() }),
        2 => Box::new(IRNodeLeaf { hrtv: true, inst: Bytecode::GET2, text: String::new() }),
        3 => Box::new(IRNodeLeaf { hrtv: true, inst: Bytecode::GET3, text: String::new() }),
        i => Box::new(IRNodeParam1 { hrtv: true, inst: Bytecode::GET, para: i, text: String::new() }),
    }
}


#[derive(Clone)]
pub struct IRNodeLetRef {
    pub(crate) info: Rc<RefCell<LetInfo>>,
    pub(crate) ref_idx: u8,
    pub(crate) hrtv: bool,
}


impl IRNode for IRNodeLetRef {
    fn as_any(&self) -> &dyn Any { self }
    fn hasretval(&self) -> bool { self.hrtv }
    fn bytecode(&self) -> u8 { Bytecode::IRLIST as u8 }
    fn codegen(&self) -> VmrtRes<Vec<u8>> {
        let (slot, expr) = {
            let info = self.info.borrow();
            (info.slot, Rc::clone(&info.expr))
        };
        if let Some(slot) = slot {
            if self.ref_idx == 0 {
                let mut codes = expr.codegen()?;
                let mut dup_codes = dup_inst_node().codegen()?;
                codes.append(&mut dup_codes);
                let mut put_codes = IRNodeParam1 {
                    hrtv: false,
                    inst: Bytecode::PUT,
                    para: slot,
                    text: String::new(),
                }.codegen()?;
                codes.append(&mut put_codes);
                return Ok(codes);
            }
            return local_get_node(slot).codegen();
        }
        expr.codegen()
    }
    fn serialize(&self) -> Vec<u8> {
        self.codegen().unwrap()
    }
    fn print(&self, suo: &str, tab: usize, desc: bool) -> String {
        let (slot, expr) = {
            let info = self.info.borrow();
            (info.slot, Rc::clone(&info.expr))
        };
        if let Some(slot) = slot {
            let mut buf = String::from(suo.repeat(tab));
            if self.ref_idx == 0 {
                let expr_str = expr.print("", 0, desc);
                buf.push_str(&format!("let ${} = {}", slot, expr_str));
            } else {
                buf.push_str(&format!("${}", slot));
            }
            buf
        } else {
            expr.print(suo, tab, desc)
        }
    }
}
