use super::rt::Bytecode;

/*************************************/

#[derive(Debug, Clone)]
pub struct IRNodeEmpty {}

impl IRNode for IRNodeEmpty {
    fn as_any(&self) -> &dyn Any { self }
    fn hasretval(&self) -> bool { false }
    fn bytecode(&self) -> u8 { 0 }
    fn codegen(&self) -> VmrtRes<Vec<u8>> { Ok(vec![]) }
    fn codegen_into(&self, _buf: &mut Vec<u8>) -> VmrtRes<()> { Ok(()) }
    fn print(&self) -> String { String::new() }
}


#[derive(Debug, Clone)]
pub struct IRNodeTopStackValue {}

impl IRNode for IRNodeTopStackValue {
    fn as_any(&self) -> &dyn Any { self }
    fn hasretval(&self) -> bool { true }

    // This node is an internal, codegen-only placeholder that represents:
    // “a value is already on the top of the stack”. It must never be serialized
    // into ircode; if it ever leaks into serialization, it would create
    // ambiguous/shifted IR streams.
    fn bytecode(&self) -> u8 { 0 }

    fn codegen(&self) -> VmrtRes<Vec<u8>> { Ok(vec![]) }
    fn codegen_into(&self, _buf: &mut Vec<u8>) -> VmrtRes<()> { Ok(()) }

    fn serialize(&self) -> Vec<u8> {
        panic!("IRNodeTopStackValue is codegen-only and cannot be serialized")
    }

    fn print(&self) -> String { "<TopStackValue>".to_string() }
}


#[derive(Debug, Clone)]
pub struct IRNodeText {
    pub text: String
}

impl IRNode for IRNodeText {
    fn as_any(&self) -> &dyn Any { self }
    fn hasretval(&self) -> bool { false }
    fn bytecode(&self) -> u8 { 0 }
    fn codegen(&self) -> VmrtRes<Vec<u8>> { Ok(vec![]) }
    fn codegen_into(&self, _buf: &mut Vec<u8>) -> VmrtRes<()> { Ok(()) }
}

impl IRNodeText {
    pub fn into_text(self) -> String {
        self.text
    }
}

/*************************************/


#[derive(Debug, Clone)]
pub struct IRNodeLeaf {
    pub hrtv: bool, 
    pub inst: Bytecode,
    pub text: String,
}

impl IRNode for IRNodeLeaf {
    fn as_any(&self) -> &dyn Any { self }
    fn hasretval(&self) -> bool { self.hrtv }
    fn bytecode(&self) -> u8 { self.inst as u8 }
    fn codegen_into(&self, buf: &mut Vec<u8>) -> VmrtRes<()> {
        buf.push(self.bytecode());
        match self.inst {
            // Keep loop-control placeholder width at 3 bytes in generated body code.
            // This guarantees that later rewrite to `JMPSL + i16` does not change size.
            Bytecode::IRBREAK | Bytecode::IRCONTINUE => buf.extend_from_slice(&[0, 0]),
            _ => {}
        }
        Ok(())
    }
    fn serialize(&self) -> Vec<u8> { vec![self.inst as u8] }
    fn print(&self) -> String {
        format!("{:?}", self.inst)
    }

}

impl IRNodeLeaf {
    pub fn nop() -> Self {
        Self { hrtv: false, inst: Bytecode::NOP, text: "".to_owned() }
    }
    pub fn notext(hrtv: bool, inst: Bytecode) -> Self {
        Self { hrtv, inst, text: "".to_owned() }
    }
    pub fn nop_box() -> Box<dyn IRNode> {
        Box::new(Self::nop())
    }
    pub fn as_text(&self) -> &String {
        &self.text
    }
}


/*************************************/


#[derive(Debug, Clone)]
pub struct IRNodeParam1 {
    pub hrtv: bool, 
    pub inst: Bytecode,
    pub para: u8,
    pub text: String,
}

impl IRNode for IRNodeParam1 {
    fn as_any(&self) -> &dyn Any { self }
    fn hasretval(&self) -> bool { self.hrtv }
    fn bytecode(&self) -> u8 { self.inst as u8 }
    fn codegen_into(&self, buf: &mut Vec<u8>) -> VmrtRes<()> {
        buf.push(self.bytecode());
        buf.push(self.para);
        Ok(())
    }
    fn serialize(&self) -> Vec<u8> {
        // IR serialized form is prefix-coded: [opcode][param...]
        vec![self.bytecode(), self.para]
    }
    fn print(&self) -> String {
        format!("{:?} {}", self.inst, self.para)
    }

}

impl IRNodeParam1 {
    pub fn into_text(self) -> String {
        self.text
    }
    pub fn to_text(&self) -> String {
        self.text.clone()
    }
    pub fn as_text(&self) -> &String {
        &self.text
    }
}


#[derive(Debug, Clone)]
pub struct IRNodeParam2 {
    pub hrtv: bool, 
    pub inst: Bytecode,
    pub para: [u8; 2],
}

impl IRNode for IRNodeParam2 {
    fn as_any(&self) -> &dyn Any { self }
    fn hasretval(&self) -> bool { self.hrtv }
    fn bytecode(&self) -> u8 { self.inst as u8 }
    fn codegen_into(&self, buf: &mut Vec<u8>) -> VmrtRes<()> {
        buf.push(self.bytecode());
        buf.extend_from_slice(&self.para);
        Ok(())
    }
    fn serialize(&self) -> Vec<u8> {
        // IR serialized form is prefix-coded: [opcode][param...]
        iter::once(self.bytecode())
            .chain(self.para)
            .collect()
    }
    fn print(&self) -> String {
        format!("{:?} {} {}", self.inst, self.para[0], self.para[1])
    }

}

#[derive(Debug, Clone)]
pub struct IRNodeParams {
    pub hrtv: bool, 
    pub inst: Bytecode,
    pub para: Vec<u8>,
}

impl IRNode for IRNodeParams {
    fn as_any(&self) -> &dyn Any { self }
    fn hasretval(&self) -> bool { self.hrtv }
    fn bytecode(&self) -> u8 { self.inst as u8 }
    fn codegen_into(&self, buf: &mut Vec<u8>) -> VmrtRes<()> {
        buf.push(self.bytecode());
        buf.extend_from_slice(&self.para);
        Ok(())
    }
    fn serialize(&self) -> Vec<u8> {
        // IR serialized form is prefix-coded: [opcode][param...]
        iter::once(self.bytecode())
            .chain(self.para.iter().copied())
            .collect()
    }
    fn print(&self) -> String {
        let parastr = hex::encode(&self.para);
        format!("{:?} 0x{}", self.inst, parastr)
    }

}




#[derive(Debug, Clone)]
pub struct IRNodeParamsSingle {
    pub hrtv: bool, 
    pub inst: Bytecode,
    pub para: Vec<u8>,
    pub subx: Box<dyn IRNode>,
}

impl IRNode for IRNodeParamsSingle {
    fn as_any(&self) -> &dyn Any { self }
    fn subs(&self) -> usize { 1 }
    fn hasretval(&self) -> bool { self.hrtv }
    fn bytecode(&self) -> u8 { self.inst as u8 }
    fn codegen_into(&self, buf: &mut Vec<u8>) -> VmrtRes<()> {
        self.subx.codegen_into(buf)?;
        buf.push(self.bytecode());
        buf.extend_from_slice(&self.para);
        Ok(())
    }
    fn serialize(&self) -> Vec<u8> {
        iter::once(self.bytecode())
        .chain(self.para.clone())
        .chain(self.subx.serialize())
        .collect()
    }
    fn print(&self) -> String {
        let parastr = hex::encode(&self.para);
        let substr = self.subx.print();
        format!("{:?} 0x{} {}", self.inst, parastr, substr)
    }


}


/*************************************/


#[derive(Debug, Clone)]
pub struct IRNodeWrapOne {
    pub node: Box<dyn IRNode>,
}

impl std::ops::Deref for IRNodeWrapOne {
    type Target = Box<dyn IRNode>;
    fn deref(&self) -> &Box<dyn IRNode> {
        &self.node
    }
}

impl std::ops::DerefMut for IRNodeWrapOne {
    fn deref_mut(&mut self) -> &mut Box<dyn IRNode> {
        &mut self.node
    }
}

impl IRNode for IRNodeWrapOne {
    fn as_any(&self) -> &dyn Any { self }
    fn subs(&self) -> usize { 1 }
    fn hasretval(&self) -> bool { self.node.hasretval() }
    fn bytecode(&self) -> u8 { self.node.bytecode() }
    fn codegen_into(&self, buf: &mut Vec<u8>) -> VmrtRes<()> {
        self.node.codegen_into(buf)
    }
    fn serialize(&self) -> Vec<u8> { self.node.serialize() }
    fn print(&self) -> String {
        format!("({})", self.node.print())
    }

}

/*************************************/


#[derive(Debug, Clone)]
pub struct IRNodeSingle {
    pub hrtv: bool, 
    pub inst: Bytecode,
    pub subx: Box<dyn IRNode>,
}

impl IRNode for IRNodeSingle {
    fn as_any(&self) -> &dyn Any { self }
    fn subs(&self) -> usize { 1 }
    fn hasretval(&self) -> bool { self.hrtv }
    fn bytecode(&self) -> u8 { self.inst as u8 }
    fn serialize(&self) -> Vec<u8> {
        iter::once(self.bytecode())
        .chain(self.subx.serialize())
        .collect()
    }
    fn print(&self) -> String {
        let substr = self.subx.print();
        format!("{:?} {}", self.inst, substr)
    }
    fn codegen_into(&self, buf: &mut Vec<u8>) -> VmrtRes<()> {
        self.subx.codegen_into(buf)?;
        buf.push(self.bytecode());
        Ok(())
    }
}


#[derive(Debug, Clone)]
pub struct IRNodeDouble {
    pub hrtv: bool, 
    pub inst: Bytecode,
    pub subx: Box<dyn IRNode>,
    pub suby: Box<dyn IRNode>,
}

impl IRNode for IRNodeDouble {
    fn as_any(&self) -> &dyn Any { self }
    fn as_any_mut(&mut self) -> &mut dyn Any { self }
    fn subs(&self) -> usize { 2 }
    fn level(&self) -> u8 {
        match OpTy::from_bytecode(self.inst) {
            Ok(t) => t.level(),
            _ => 0,
        }
    }
    fn hasretval(&self) -> bool { self.hrtv }
    fn bytecode(&self) -> u8 { self.inst as u8 }
    fn codegen_into(&self, buf: &mut Vec<u8>) -> VmrtRes<()> {
        if let Some(c) = compile_double(self.inst, &self.subx, &self.suby)? {
            buf.extend(c);
            return Ok(());
        }
        self.subx.codegen_into(buf)?;
        self.suby.codegen_into(buf)?;
        buf.push(self.bytecode());
        Ok(())
    }
    fn serialize(&self) -> Vec<u8> {
        iter::once(self.bytecode())
        .chain(self.subx.serialize())
        .chain(self.suby.serialize())
        .collect::<Vec<u8>>()
    }
    fn print(&self) -> String {
        let subxstr = self.subx.print();
        let subystr = self.suby.print();
        format!("{:?} {} {}", self.inst, subxstr, subystr)
    }

}

#[derive(Debug, Clone)]
pub struct IRNodeTriple {
    pub hrtv: bool, 
    pub inst: Bytecode,
    pub subx: Box<dyn IRNode>,
    pub suby: Box<dyn IRNode>,
    pub subz: Box<dyn IRNode>,
}

impl IRNode for IRNodeTriple {
    fn as_any(&self) -> &dyn Any { self }
    fn subs(&self) -> usize { 3 }
    fn hasretval(&self) -> bool { self.hrtv }
    fn bytecode(&self) -> u8 { self.inst as u8 }
    fn codegen_into(&self, buf: &mut Vec<u8>) -> VmrtRes<()> {
        if let Some(c) = compile_triple(self.inst, &self.subx, &self.suby, &self.subz)? {
            buf.extend(c);
            return Ok(());
        }
        self.subx.codegen_into(buf)?;
        self.suby.codegen_into(buf)?;
        self.subz.codegen_into(buf)?;
        buf.push(self.bytecode());
        Ok(())
    }
    fn serialize(&self) -> Vec<u8> {
        iter::once(self.bytecode())
        .chain(self.subx.serialize())
        .chain(self.suby.serialize())
        .chain(self.subz.serialize())
        .collect()
    }
    fn print(&self) -> String {
        let subxstr = self.subx.print();
        let subystr = self.suby.print();
        let subzstr = self.subz.print();
        format!("{:?} {} {} {}", self.inst, subxstr, subystr, subzstr)
    }

}


/*************************************/

#[derive(Debug, Clone)]
pub struct IRNodeParam1Single {
    pub hrtv: bool, 
    pub inst: Bytecode,
    pub para: u8,
    pub subx: Box<dyn IRNode>,
}

impl IRNode for IRNodeParam1Single {
    fn as_any(&self) -> &dyn Any { self }
    fn subs(&self) -> usize { 1 }
    fn hasretval(&self) -> bool { self.hrtv }
    fn bytecode(&self) -> u8 { self.inst as u8 }
    fn codegen_into(&self, buf: &mut Vec<u8>) -> VmrtRes<()> {
        self.subx.codegen_into(buf)?;
        buf.push(self.bytecode());
        buf.push(self.para);
        Ok(())
    }
    fn serialize(&self) -> Vec<u8> {
        iter::once(self.bytecode())
        .chain([self.para])
        .chain(self.subx.serialize())
        .collect()
    }
    fn print(&self) -> String {
        let substr = self.subx.print();
        format!("{:?} {} {}", self.inst, self.para, substr)
    }
}


#[derive(Debug, Clone)]
pub struct IRNodeParam2Single {
    pub hrtv: bool, 
    pub inst: Bytecode,
    pub para: [u8; 2],
    pub subx: Box<dyn IRNode>,
}

impl IRNode for IRNodeParam2Single {
    fn as_any(&self) -> &dyn Any { self }
    fn subs(&self) -> usize { 1 }
    fn hasretval(&self) -> bool { self.hrtv }
    fn bytecode(&self) -> u8 { self.inst as u8 }
    fn codegen_into(&self, buf: &mut Vec<u8>) -> VmrtRes<()> {
        self.subx.codegen_into(buf)?;
        buf.push(self.bytecode());
        buf.extend_from_slice(&self.para);
        Ok(())
    }
    fn serialize(&self) -> Vec<u8> {
        iter::once(self.bytecode())
        .chain(self.para)
        .chain(self.subx.serialize())
        .collect()
    }
    fn print(&self) -> String {
        let parastr = hex::encode(&self.para);
        let substr = self.subx.print();
        format!("{:?} 0x{} {}", self.inst, parastr, substr)
    }
}



/*************************************/


#[derive(Default, Debug, Clone)]
pub struct IRNodeBytecodes {
    pub codes: Vec<u8>,
}

impl IRNode for IRNodeBytecodes {
    fn as_any(&self) -> &dyn Any { self }
    fn subs(&self) -> usize { 0 }
    fn hasretval(&self) -> bool { false }
    fn bytecode(&self) -> u8 { IRBYTECODE as u8 }
    fn codegen(&self) -> VmrtRes<Vec<u8>> {
        Ok(self.codes.clone())
    }
    fn codegen_into(&self, buf: &mut Vec<u8>) -> VmrtRes<()> {
        buf.extend_from_slice(&self.codes);
        Ok(())
    }
    fn serialize(&self) -> Vec<u8> {
        if self.codes.len() > u16::MAX as usize {
            panic!("IRNodeBytecodes payload too long");
        }
        iter::once(IRBYTECODE as u8)
            .chain((self.codes.len() as u16).to_be_bytes())
            .chain(self.codes.clone())
            .collect::<Vec<_>>()
    }
    fn print(&self) -> String {
        let codes = match self.codes.bytecode_print(false) {
            Ok(s) => s.trim_end().to_owned(),
            Err(_) => format!("0x{}", hex::encode(&self.codes)),
        };
        format!("bytecode {{ {} }}", codes)
    }
}


/*************************************/

#[derive(Debug, Clone)]
pub struct IRNodeArray {
    pub subs: Vec<Box<dyn IRNode>>,
    pub inst: Bytecode,
}

impl Default for IRNodeArray {
    fn default() -> Self {
        Self::new_list()
    }
}

impl std::ops::Deref for IRNodeArray {
    type Target = Vec<Box<dyn IRNode>>;
    fn deref(&self) -> &Vec<Box<dyn IRNode>> {
        &self.subs
    }
}

impl std::ops::DerefMut for IRNodeArray {
    fn deref_mut(&mut self) -> &mut Vec<Box<dyn IRNode>> {
        &mut self.subs
    }
}

impl IRNode for IRNodeArray {
    fn as_any(&self) -> &dyn Any { self }
    fn subs(&self) -> usize { self.subs.len() }
    fn hasretval(&self) -> bool {
        match self.inst {
            // Statement blocks compile by popping any produced values; they do not yield a value.
            Bytecode::IRBLOCK => false,
            _ => match self.subs.last() {
                None => false,
                Some(s) => s.hasretval(),
            },
        }
    }
    fn bytecode(&self) -> u8 { self.inst as u8 }
    fn codegen(&self) -> VmrtRes<Vec<u8>> {
        match self.inst {
            Bytecode::IRLIST => compile_list(&self.subs),
            Bytecode::IRBLOCK | Bytecode::IRBLOCKR => compile_block(self.inst, &self.subs),
            _ => errf!("IRNodeArray invalid opcode {:?}", self.inst).map_ire(InstInvalid)
        }
    }
    fn codegen_into(&self, buf: &mut Vec<u8>) -> VmrtRes<()> {
        match self.inst {
            Bytecode::IRLIST => compile_list_into(&self.subs, buf),
            Bytecode::IRBLOCK | Bytecode::IRBLOCKR => compile_block_into(self.inst, &self.subs, buf),
            _ => errf!("IRNodeArray invalid opcode {:?}", self.inst).map_ire(InstInvalid),
        }
    }
    fn serialize(&self) -> Vec<u8> {
        if self.subs.len() > u16::MAX as usize {
            panic!("IRNode list or block length overflow")
        }
        let mut children_bytes: Vec<u8> = Vec::new();
        let mut count: usize = 0;
        for a in &self.subs {
            let mut b = a.serialize();
            if !b.is_empty() {
                count += 1;
                children_bytes.append(&mut b);
            }
        }
        let mut bytes = iter::once(self.inst as u8)
            .chain((count as u16).to_be_bytes()).collect::<Vec<_>>();
        bytes.append(&mut children_bytes);
        bytes
    }
    fn print(&self) -> String {
        let num = self.subs.len();
        if num == 0 {
            return format!("{:?} {}:", self.inst, num);
        }
        let mut buf = format!("{:?} {}:\n", self.inst, num);
        for a in &self.subs {
            buf.push_str(&a.print());
            buf.push('\n');
        }
        buf.pop();
        buf
    }
}

impl IRNodeArray {
    pub fn new_list() -> Self {
        Self {
            subs: vec![],
            inst: Bytecode::IRLIST,
        }
    }
    pub fn new_block() -> Self {
        Self {
            subs: vec![],
            inst: Bytecode::IRBLOCK,
        }
    }
    pub fn new_block_expr() -> Self {
        Self {
            subs: vec![],
            inst: Bytecode::IRBLOCKR,
        }
    }
    pub fn with_opcode(inst: Bytecode) -> Self {
        Self {
            subs: vec![],
            inst,
        }
    }
    pub fn with_capacity(n: usize, inst: Bytecode) -> Ret<Self> {
        if n > u16::MAX as usize {
            return errf!("IRNodeArray length max {}", u16::MAX)
        }
        Ok(Self{
            subs: Vec::with_capacity(n),
            inst,
        })
    }
    pub fn from_vec(subs: Vec<Box<dyn IRNode>>, inst: Bytecode) -> Ret<Self> {
        if subs.len() > u16::MAX as usize {
            return errf!("IRNodeArray length max {}", u16::MAX)
        }
        Ok(Self{subs, inst})
    }
    pub fn into_vec(self) -> Vec<Box<dyn IRNode>> {
        self.subs
    }
    pub fn into_one(mut self) -> Box<dyn IRNode> {
        self.subs.pop().unwrap()
    }
    pub fn set_inst(&mut self, inst: Bytecode) {
        self.inst = inst;
    }
}

#[cfg(test)]
mod node_tests {
    use super::*;

    #[test]
    fn wrap_codegen_delegates_param_node() {
        let wrapped: Box<dyn IRNode> = Box::new(IRNodeWrapOne {
            node: Box::new(IRNodeParam1 {
                hrtv: true,
                inst: Bytecode::PU8,
                para: 100,
                text: String::new(),
            }),
        });
        let bytes = wrapped.codegen().expect("codegen");
        assert_eq!(bytes, vec![Bytecode::PU8 as u8, 100]);
    }

    #[test]
    fn wrap_codegen_delegates_compound_node() {
        let wrapped: Box<dyn IRNode> = Box::new(IRNodeWrapOne {
            node: Box::new(IRNodeDouble {
                hrtv: true,
                inst: Bytecode::ADD,
                subx: Box::new(IRNodeLeaf::notext(true, Bytecode::P1)),
                suby: Box::new(IRNodeLeaf::notext(true, Bytecode::P2)),
            }),
        });
        let bytes = wrapped.codegen().expect("codegen");
        assert_eq!(
            bytes,
            vec![Bytecode::P1 as u8, Bytecode::P2 as u8, Bytecode::ADD as u8]
        );
    }
}
