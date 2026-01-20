
struct LetInfo {
    expr: Rc<Box<dyn IRNode>>,
    slot: Option<u8>,
    refs: u8,
    needs_slot: bool,
}

impl LetInfo {
    fn new(node: Box<dyn IRNode>) -> Self {
        Self {
            expr: Rc::new(node),
            slot: None,
            refs: 0,
            needs_slot: false,
        }
    }
}


#[derive(Clone)]
struct IRNodeLetRef {
    info: Rc<RefCell<LetInfo>>,
    ref_idx: u8,
    hrtv: bool,
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
                let mut dup_codes = Syntax::push_inst(Bytecode::DUP).codegen()?;
                codes.append(&mut dup_codes);
                let mut put_codes = IRNodeParam1{
                    hrtv: false,
                    inst: Bytecode::PUT,
                    para: slot,
                    text: s!(""),
                }.codegen()?;
                codes.append(&mut put_codes);
                return Ok(codes);
            }
            return Syntax::push_local_get(slot, s!("")).codegen();
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

