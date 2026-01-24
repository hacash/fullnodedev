use crate::PrintOption;
use native::NativeCall;
use super::rt::{Bytecode, SourceMap};
use super::value::ValueTy;



macro_rules! print_sub {
    ($opt:expr, $subx:expr) => {
        match $subx.subs() {
            0 => $subx.print($opt),
            _ => {
                let child_opt = $opt.child();
                let mut buf = s!("\n") + &$subx.print(&child_opt);
                if $opt.desc {
                    buf += &(s!("\n") + &$opt.indent.repeat($opt.tab));
                }
                buf
            }
        }
    };
}

macro_rules! print_sub_newline {
    ($opt:expr, $subx:expr) => {{
        let child_opt = $opt.child();
        let sub = $subx.print(&child_opt);
        let emp = sub.replace(" ", "").replace("\n", "");
        match emp.len() > 0 {
            true => {
                let mut buf = s!("\n") + &sub;
                if $opt.desc {
                    buf += &(s!("\n") + &$opt.indent.repeat($opt.tab));
                }
                buf
            },
            false => emp,
        }
    }}
}

macro_rules! print_sub_inline {
    ($opt:expr, $subx:expr) => {{
        let inline_opt = $opt.with_tab(0);
        let substr = print_sub!(&inline_opt, $subx);
        match substr.contains('\n') {
            true => substr[(inline_opt.indent.len()+1)..substr.len()-1].to_owned(),
            false => substr,
        }
    }};
}


macro_rules! print_subx_suby_op {
    ($opt:expr, $buf:ident, $self:ident, $op:expr) => {
        {
            let wrapx = $self.subx.as_any().downcast_ref::<IRNodeWrapOne>().is_some();
            let wrapy = $self.suby.as_any().downcast_ref::<IRNodeWrapOne>().is_some();
            let inline_opt = $opt.with_tab(0);
            let mut subx = print_sub_inline!(&inline_opt, $self.subx);
            let mut suby = print_sub_inline!(&inline_opt, $self.suby);
            let clv = match OpTy::from_bytecode($self.inst) {
                Ok(t) => t.level(),
                _ => 0,
            };
            let llv = $self.subx.level();
            let rlv = $self.suby.level();
            if clv>0 && llv>0 && clv>llv && !wrapx {
                subx = format!("({})", &subx);
            }
            let need_wrap_right = clv>0 && rlv>0 && !wrapy && (clv>rlv || clv==rlv);
            if need_wrap_right {
                suby = format!("({})", &suby);
            }
            let res = format!("{} {} {}", &subx, $op, &suby);
            $buf.push_str(&res);
        }
    };
}


/*************************************/

fn slot_name_display(slot: u8, map: Option<&SourceMap>) -> String {
    map.and_then(|m| m.slot(slot))
        .cloned()
        .unwrap_or_else(|| format!("${}", slot))
}




#[derive(Debug, Clone)]
pub struct IRNodeEmpty {}

impl IRNode for IRNodeEmpty {
    fn as_any(&self) -> &dyn Any { self }
    fn hasretval(&self) -> bool { false }
    fn bytecode(&self) -> u8 { 0 }
    fn codegen(&self) -> VmrtRes<Vec<u8>> { Ok(vec![]) }
    fn codegen_into(&self, _buf: &mut Vec<u8>) -> VmrtRes<()> { Ok(()) }
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

fn format_call_args(substr: &str) -> String {
    let lines: Vec<String> = substr
        .lines()
        .map(|line| line.trim())
        .filter(|line| !line.is_empty())
        .map(|line| line.to_string())
        .collect();
    lines.join(", ")
}

fn collect_native_call_args(node: &dyn IRNode, opt: &PrintOption) -> Vec<String> {
    let mut args = Vec::new();
    let mut current: &dyn IRNode = node;
    loop {
        if let Some(list) = current.as_any().downcast_ref::<IRNodeList>() {
            if let Some(elements) = extract_packlist_elements(list.inst, &list.subs, opt) {
                args.extend(elements);
                return args;
            }
        }
        if let Some(double) = current.as_any().downcast_ref::<IRNodeDouble>() {
            if double.inst == Bytecode::CAT {
                args.push(print_sub_inline!(opt, double.subx));
                current = &*double.suby;
                continue;
            }
        }
        args.push(print_sub_inline!(opt, current));
        break;
    }
    args
}

fn trim_nil_args(opt: &PrintOption, args: &mut Vec<String>, node: &dyn IRNode) {
    if opt.hide_func_nil_argv && node.bytecode() == Bytecode::PNIL as u8 {
        args.clear();
    }
}

fn build_call_args(opt: &PrintOption, node: &dyn IRNode) -> String {
    let mut args_list = collect_native_call_args(node, opt);
    trim_nil_args(opt, &mut args_list, node);
    let args_src = args_list.join("\n");
    format_call_args(&args_src)
}

fn extract_const_usize(node: &dyn IRNode) -> Option<usize> {
    use Bytecode::*;
    if let Some(leaf) = node.as_any().downcast_ref::<IRNodeLeaf>() {
        return match leaf.inst {
            P0 => Some(0),
            P1 => Some(1),
            P2 => Some(2),
            P3 => Some(3),
            _ => None,
        };
    }
    if let Some(param1) = node.as_any().downcast_ref::<IRNodeParam1>() {
        if param1.inst == PU8 {
            return Some(param1.para as usize);
        }
    }
    if let Some(param2) = node.as_any().downcast_ref::<IRNodeParam2>() {
        if param2.inst == PU16 {
            return Some(u16::from_be_bytes(param2.para) as usize);
        }
    }
    if let Some(single) = node.as_any().downcast_ref::<IRNodeSingle>() {
        match single.inst {
            CU32 | CU64 | CU128 => {
                if let Some(params) = single.subx.as_any().downcast_ref::<IRNodeParams>() {
                    let para = &params.para;
                    if para.is_empty() {
                        return None;
                    }
                    let len = para[0] as usize;
                    if len > para.len().saturating_sub(1) {
                        return None;
                    }
                    if len == 0 {
                        return Some(0);
                    }
                    let mut value = 0u128;
                    for &b in &para[1..=len] {
                        value = (value << 8) | b as u128;
                    }
                    return Some(value as usize);
                }
                return None
            }
            _ => return None,
        }
    }
    None
}

fn extract_packlist_elements(inst: Bytecode, subs: &[Box<dyn IRNode>], opt: &PrintOption) -> Option<Vec<String>> {
    use Bytecode::*;
    if inst != IRLIST {
        return None;
    }
    let num = subs.len();
    if num < 2 {
        return None;
    }
    let last = &subs[num - 1];
    if let Some(leaf) = last.as_any().downcast_ref::<IRNodeLeaf>() {
        if leaf.inst != PACKLIST {
            return None;
        }
    } else {
        return None;
    }
    let count_idx = num - 2;
    let count = num - 2;
    let expected = extract_const_usize(&*subs[count_idx])?;
    if expected != count {
        return None;
    }
    let mut elems = Vec::with_capacity(count);
    for node in &subs[..count] {
        elems.push(print_sub_inline!(opt, &**node));
    }
    Some(elems)
}

enum TypeCheck {
    Named(&'static str),
    Unknown(u8),
}

fn resolve_type_check_name(ty: u8) -> TypeCheck {
    match ValueTy::build(ty) {
        Ok(vt) => TypeCheck::Named(vt.name()),
        Err(_) => TypeCheck::Unknown(ty),
    }
}

fn format_is_components(opt: &PrintOption, node: &dyn IRNode) -> Option<(String, TypeCheck)> {
    if let Some(inner) = node.as_any().downcast_ref::<IRNodeSingle>() {
        let target = print_sub_inline!(opt, inner.subx);
        return match inner.inst {
            Bytecode::TNIL => Some((target, TypeCheck::Named("nil"))),
            Bytecode::TLIST => Some((target, TypeCheck::Named("list"))),
            Bytecode::TMAP => Some((target, TypeCheck::Named("map"))),
            _ => None,
        };
    }
    if let Some(inner) = node.as_any().downcast_ref::<IRNodeParam1Single>() {
        if inner.inst == Bytecode::TIS {
            let target = print_sub_inline!(opt, inner.subx);
            return Some((target, resolve_type_check_name(inner.para)));
        }
    }
    None
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
    fn serialize(&self) -> Vec<u8> { vec![self.inst as u8] }
    fn print(&self, opt: &PrintOption) -> String {
        let mut buf = String::from(opt.indent.repeat(opt.tab));
        if opt.desc {
            let meta = self.inst.metadata();
            match self.inst {
                NOP => {}, 
                GET3 => buf.push_str(&slot_name_display(3, opt.map)),
                GET2 => buf.push_str(&slot_name_display(2, opt.map)),
                GET1 => buf.push_str(&slot_name_display(1, opt.map)),
                GET0 => buf.push_str(&slot_name_display(0, opt.map)),
                P3 => buf.push('3'),
                P2 => buf.push('2'),
                P1 => buf.push('1'),
                P0 => buf.push('0'),
                PNBUF => buf.push_str("\"\""),
                ABT | END => buf.push_str(meta.intro),
                _ => {
                    buf.push_str(meta.intro);
                    buf.push_str("()");
                }
            };
        } else {
            buf.push_str(&format!("{:?}", self.inst));
        }
        buf
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
        self.codegen().unwrap()
    }
    fn print(&self, opt: &PrintOption) -> String {
        let mut buf = String::from(opt.indent.repeat(opt.tab));
        if opt.desc {
            let meta = self.inst.metadata();
            match self.inst {
                PU8 =>  buf.push_str(&format!("{}", self.para)),
                GET => buf.push_str(&format!("${}", self.para)),
                EXTENV => {
                    let ary = CALL_EXTEND_ENV_DEFS;
                    let f = search_ext_name_by_id(self.para, &ary);
                    buf.push_str(&format!("{}()", f));
                },
                _ => {
                    buf.push_str(&format!("{}({})", meta.intro, self.para));
                }
            };
        } else {
            buf.push_str(&format!("{:?} {}", self.inst, self.para));
        }
        buf
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
        self.codegen().unwrap()
    }
    fn print(&self, opt: &PrintOption) -> String {
        let mut buf = String::from(opt.indent.repeat(opt.tab));
        if opt.desc {
            let meta = self.inst.metadata();
            match self.inst {
                PU16 => buf.push_str(&format!("{}", u16::from_be_bytes(self.para))) ,
                _ => {
                    let para = hex::encode(self.para);
                    buf.push_str(&format!("{}(0x{})", meta.intro, para));
                }
            };
        } else {
            buf.push_str(&format!("{:?} {} {}", self.inst, self.para[0], self.para[1]));
        }
        buf
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
        self.codegen().unwrap()
    }
    fn print(&self, opt: &PrintOption) -> String {
        let mut buf = String::from(opt.indent.repeat(opt.tab));
        let parastr = hex::encode(&self.para);
        if opt.desc {
            let meta = self.inst.metadata();
            match self.inst {
                PBUF | PBUFL => print_data_bytes(self, &mut buf),
                CALLCODE => {
                    let i = self.para[0];
                    let f = hex::encode(&self.para[1..]);
                    buf.push_str(&format!("callcode {}::{}", i, f));
                }
                _ => {
                    buf.push_str(&format!("{}(0x{})", meta.intro, parastr));
                }
            }
        } else {
            buf.push_str(&format!("{:?} 0x{}", self.inst, parastr));
        }
        buf
    }

}

fn print_data_bytes(this: &IRNodeParams, buf: &mut String) {
    let l = maybe!(PBUF==this.inst, 1, 2) as usize;
    let data = this.para[l..].to_vec();
    if let Some(s) = ascii_show_string(&data) {
        buf.push_str(&format!("\"{}\"", literals(s)));
        return
    }
    // check address
    if data.len() == Address::SIZE {
        let addr = Address::must_vec(data.clone());
        if let Ok(..) = addr.check_version() {
            buf.push_str(&format!("{}", addr.readable()));
            return 
        }
    }
    // normal data
    buf.push_str(&format!("0x{}", hex::encode(&data)));
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
    fn print(&self, opt: &PrintOption) -> String {
        let mut buf = String::from(opt.indent.repeat(opt.tab));
        let parastr = hex::encode(&self.para);
        if opt.desc {
            let meta = self.inst.metadata();
            let substr = print_sub_inline!(opt, self.subx);
            if opt.map.is_none() {
                eprintln!("debug: map none for inst {:?}", meta.intro);
            }
            let args = build_call_args(opt, &*self.subx);
            match self.inst {
                CALL => {
                    if let Some((lib, func)) = self.resolve_lib_func(opt.map) {
                        buf.push_str(&format!("{}.{}({})", lib, func, args));
                    } else {
                        let idx = self.para[0];
                        let f = hex::encode(&self.para[1..]);
                        buf.push_str(&format!("call {}::{}({})", idx, f, args));
                    }
                }
                CALLLIB => {
                    if let Some((lib, func)) = self.resolve_lib_func(opt.map) {
                        buf.push_str(&format!("{}:{}({})", lib, func, args));
                    } else {
                        let idx = self.para[0];
                        let f = hex::encode(&self.para[1..]);
                        buf.push_str(&format!("calllib {}:{}({})", idx, f, args));
                    }
                }
                CALLINR => {
                    if let Some(func) = self.resolve_self_func(opt.map) {
                        buf.push_str(&format!("self.{}({})", func, args));
                    } else {
                        let f = hex::encode(&self.para);
                        buf.push_str(&format!("callinr {}({})", f, args));
                    }
                }
                CALLSTATIC => {
                    if let Some((lib, func)) = self.resolve_lib_func(opt.map) {
                        buf.push_str(&format!("{}::{}({})", lib, func, args));
                    } else {
                        let idx = self.para[0];
                        let f = hex::encode(&self.para[1..]);
                        buf.push_str(&format!("callstatic {}::{}({})", idx, f, args));
                    }
                }
                _ => {
                    buf.push_str(&format!("{}(0x{}, {})", meta.intro, parastr, substr));
                }
            }
        } else {
            let substr = print_sub!(opt, self.subx);
            buf.push_str(&format!("{:?} 0x{} {}", self.inst, parastr, substr));
        }
        buf
    }


}


impl IRNodeParamsSingle {
    
    fn decode_lib_sig(&self) -> Option<(u8, [u8;4])> {
        if self.para.len() < 5 {
            return None;
        }
        let idx = self.para[0];
        let mut sig = [0u8; 4];
        sig.copy_from_slice(&self.para[1..5]);
        Some((idx, sig))
    }

    fn resolve_lib_func(&self, map: Option<&SourceMap>) -> Option<(String, String)> {
        let (idx, sig) = self.decode_lib_sig()?;
        let map = map?;
        let lib = map.lib(idx)?;
        let func = map.func(&sig)?;
        Some((lib.name.clone(), func.clone()))
    }

    fn decode_self_sig(&self) -> Option<[u8;4]> {
        if self.para.len() != 4 {
            return None;
        }
        let mut sig = [0u8; 4];
        sig.copy_from_slice(&self.para);
        Some(sig)
    }

    fn resolve_self_func(&self, map: Option<&SourceMap>) -> Option<String> {
        let sig = self.decode_self_sig()?;
        let map = map?;
        map.func(&sig).cloned()
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
    fn subs(&self) -> usize { 0 }
    fn hasretval(&self) -> bool { self.node.hasretval() }
    fn bytecode(&self) -> u8 { self.node.bytecode() }
    fn serialize(&self) -> Vec<u8> { self.node.serialize() }
    fn print(&self, opt: &PrintOption) -> String {
        let res = self.node.print(opt);
        format!("({})", res)
    }

}

/*************************************/

fn wrap_cast_operand(node: &dyn IRNode, text: String) -> String {
    if node.level() > 0 {
        let trimmed = text.trim();
        if trimmed.starts_with('(') && trimmed.ends_with(')') {
            text
        } else {
            format!("({})", text)
        }
    } else {
        text
    }
}




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
    fn print(&self, opt: &PrintOption) -> String {
        let mut buf = String::from(opt.indent.repeat(opt.tab));
        if opt.desc {
            let meta = self.inst.metadata();
            match self.inst {
                TNIL | TLIST | TMAP => {
                    let substr = print_sub_inline!(opt, self.subx);
                    match self.inst {
                        TNIL  => buf.push_str(&format!("{} is nil", substr)),
                        TLIST => buf.push_str(&format!("{} is list", substr)),
                        TMAP  => buf.push_str(&format!("{} is map", substr)),
                        _ => never!()
                    }
                }
                CU8 | CU16 | CU32 | CU64 | CU128 | NOT | RET | ERR | AST => {
                    let substr = print_sub_inline!(opt, self.subx);
                    let operand = wrap_cast_operand(&*self.subx, substr.clone());
                    match self.inst {
                        CU8   => buf.push_str(&format!("{} as u8", operand)),
                        CU16  => buf.push_str(&format!("{} as u16", operand)),
                        CU32  => buf.push_str(&format!("{} as u32", operand)),
                        CU64  => buf.push_str(&format!("{} as u64", operand)),
                        CU128 => buf.push_str(&format!("{} as u128", operand)),
                        NOT   => {
                            if let Some((target, ty)) = format_is_components(opt, &*self.subx) {
                                match ty {
                                    TypeCheck::Named(name) => buf.push_str(&format!("{} is not {}", target, name)),
                                    TypeCheck::Unknown(id) => buf.push_str(&format!("type_id({}) != {}", target, id)),
                                }
                            } else {
                                buf.push_str(&format!("! {}", substr));
                            }
                        }
                        RET | ERR | AST => buf.push_str(&format!("{} {}", meta.intro, substr)),
                        _ => never!()
                    };
                },
                _ => {
                    let substr = print_sub_inline!(opt, self.subx);
                    match self.inst {
                        Bytecode::PRT => buf.push_str(&format!("{} {}", meta.intro, substr)),
                        _ => buf.push_str(&format!("{}({})", meta.intro, substr)),
                    }
                }
            };
        } else {
            let substr = print_sub!(opt, self.subx);
            buf.push_str(&format!("{:?} {}", self.inst, substr));
        }
        buf
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
    fn print(&self, opt: &PrintOption) -> String {
        let mut buf = String::from(opt.indent.repeat(opt.tab));
        if opt.desc {
            let meta = self.inst.metadata();
            match self.inst {
                ADD 
                | SUB 
                | MUL 
                | DIV 
                | POW 
                | MOD 
                | GT  
                | LT  
                | GE  
                | LE  
                | AND 
                | OR  
                | EQ  
                | NEQ 
                | BAND
                | BOR 
                | BXOR
                | BSHL
                | BSHR
                | CAT 
                => {
                    let sg = OpTy::from_bytecode(self.inst).unwrap().symbol();
                    print_subx_suby_op!(opt, buf, self, sg)
                }
                IRWHILE => {
                    let subxstr = print_sub_inline!(opt, self.subx);
                    let subystr = print_sub!(opt, self.suby);
                    buf.push_str(&format!("while {} {{{}}}", subxstr, subystr))
                }
                ITEMGET => {
                    let subxstr = print_sub!(opt, self.subx);
                    let subystr = print_sub_inline!(opt, self.suby);
                    buf.push_str(&format!("{}[{}]", subxstr, subystr))
                }
                _ => {
                    let subxstr = print_sub!(opt, self.subx);
                    let subystr = print_sub!(opt, self.suby);
                    buf.push_str(&format!("{}({}, {})", meta.intro, subxstr, subystr))
                }
            }
        } else {
            let subxstr = print_sub!(opt, self.subx);
            let subystr = print_sub!(opt, self.suby);
            buf.push_str(&format!("{:?} {} {}", self.inst, subxstr, subystr));
        }
        buf
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
    fn print(&self, opt: &PrintOption) -> String {
        let mut buf = String::from(opt.indent.repeat(opt.tab));
        if opt.desc {
            let meta = self.inst.metadata();
            match self.inst {
                IRIF | IRIFR => {
                    let subxstr = &print_sub_inline!(opt, self.subx);
                    let subystr = print_sub_newline!(opt, self.suby);
                    let subzstr = print_sub_newline!(opt, self.subz);
                    buf.push_str(&format!("if {} {{{}}}", subxstr, subystr));
                    if subzstr.len() > 0 {
                        buf.push_str(&format!(" else {{{}}}", subzstr));
                    }
                }
                _ => {
                    let subxstr = print_sub!(opt, self.subx);
                    let subystr = print_sub!(opt, self.suby);
                    let subzstr = print_sub!(opt, self.subz);
                    buf.push_str(&format!("{}({}, {}, {})", meta.intro, subxstr, subystr, subzstr));
                }
            }
        } else {
            let subxstr = print_sub!(opt, self.subx);
            let subystr = print_sub!(opt, self.suby);
            let subzstr = print_sub!(opt, self.subz);
            buf.push_str(&format!("{:?} {} {} {}", self.inst, subxstr, subystr, subzstr));
        }
        buf
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
    fn print(&self, opt: &PrintOption) -> String {
        let mut buf = String::from(opt.indent.repeat(opt.tab));
        let substr = print_sub!(opt, self.subx);
        if opt.desc {
            let meta = self.inst.metadata();
            match self.inst {
                TIS => {
                    let substr = print_sub_inline!(opt, self.subx);
                    match resolve_type_check_name(self.para) {
                        TypeCheck::Named(name) => buf.push_str(&format!("{} is {}", substr, name)),
                        TypeCheck::Unknown(id) => buf.push_str(&format!("type_id({}) == {}", substr, id)),
                    }
                }
                PUT => {
                    let substr = &print_sub_inline!(opt, self.subx);
                    let slot_name = opt.map.and_then(|s| s.slot(self.para)).cloned();
                    let is_first = opt.mark_slot_put(self.para);
                    let target = if is_first {
                        match slot_name {
                            Some(name) => format!("var {} ${}", name, self.para),
                            None => format!("var ${}", self.para),
                        }
                    } else {
                        slot_name.unwrap_or_else(|| format!("${}", self.para))
                    };
                    let line = format!("{} = {}", target, substr);
                    buf.push_str(&line);
                }
                XOP => {
                    let substr = &print_sub_inline!(opt, self.subx);
                    let (op_str, idx) = local_operand_param_parse(self.para);
                    let target = slot_name_display(idx, opt.map);
                    let line = format!("{} {} {}", target, op_str, substr);
                    buf.push_str(&line);
                }
                XLG => {
                    let substr = &print_sub_inline!(opt, self.subx);
                    let (opt_str, idx) = local_logic_param_parse(self.para);
                    let target = slot_name_display(idx, opt.map);
                    let line = format!("{} {} {}", target, opt_str, substr);
                    buf.push_str(&line);
                }
                EXTFUNC => {
                    let substr = &print_sub_inline!(opt, self.subx);
                    let ary = CALL_EXTEND_FUNC_DEFS;
                    let f = search_ext_name_by_id(self.para, &ary);
                    buf.push_str(&format!("{}({})", f, substr));
                }
                EXTACTION => {
                    let args = build_call_args(opt, &*self.subx);
                    let ary = CALL_EXTEND_ACTION_DEFS;
                    let f = search_ext_name_by_id(self.para, &ary);
                    buf.push_str(&format!("{}({})", f, args));
                }
                NTCALL => {
                    let args = build_call_args(opt, &*self.subx);
                    let ntcall: NativeCall = std_mem_transmute!(self.para);
                    buf.push_str(&format!("{}({})", ntcall.name(), args));
                }
                _ => {
                    buf.push_str(meta.intro);
                    buf.push_str(&format!("({}, {})", self.para, substr));
                }
            };
        } else {
            buf.push_str(&format!("{:?} {} {}", self.inst, self.para, substr));
        }
        buf
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
    fn print(&self, opt: &PrintOption) -> String {
        let mut buf = String::from(opt.indent.repeat(opt.tab));
        let parastr = hex::encode(&self.para);
        let substr = print_sub!(opt, self.subx);
        if opt.desc {
            let meta = self.inst.metadata();
            buf.push_str(meta.intro);
            buf.push_str(&format!("(0x{}, {})", parastr, substr));
        } else {
            buf.push_str(&format!("{:?} 0x{} {}", self.inst, parastr, substr));
        }
        buf
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
    fn print(&self, opt: &PrintOption) -> String {
        let pre = opt.indent.repeat(opt.tab);
        let codes = self.codes.bytecode_print(false).unwrap();
        let codes = codes.trim_end();
        format!("{}bytecode {{ {} }}", pre, codes)
    }
}


/*************************************/

macro_rules! define_ir_list_or_block { ($name: ident, $inst: expr, $compile_fn: ident) => {
     

#[derive(Debug, Clone)]
pub struct $name {
    pub subs: Vec<Box<dyn IRNode>>,
    pub inst: Bytecode,
}

impl Default for $name {
    fn default() -> Self {
        Self::new()
    }
}

impl std::ops::Deref for $name {
    type Target = Vec<Box<dyn IRNode>>;
    fn deref(&self) -> &Vec<Box<dyn IRNode>> {
        &self.subs
    }
}

impl DerefMut for $name {
    fn deref_mut(&mut self) -> &mut Vec<Box<dyn IRNode>> {
        &mut self.subs
    }
}

impl IRNode for $name {
    fn as_any(&self) -> &dyn Any { self }
    fn subs(&self) -> usize { self.subs.len() }
    fn hasretval(&self) -> bool {
        match self.subs.last() {
            None => false,
            Some(s) => s.hasretval(),
        }
    }
    fn bytecode(&self) -> u8 { self.inst as u8 }
    fn codegen(&self) -> VmrtRes<Vec<u8>> {
        $compile_fn(self.inst, &self.subs)
    }
    fn codegen_into(&self, buf: &mut Vec<u8>) -> VmrtRes<()> {
        let codes = $compile_fn(self.inst, &self.subs)?;
        buf.extend_from_slice(&codes);
        Ok(())
    }
    fn serialize(&self) -> Vec<u8> {
        if self.subs.len() > u16::MAX as usize {
            panic!("IRNode list or block length overflow")
        }
        let mut bytes = iter::once(self.inst as u8)
            .chain((self.subs.len() as u16).to_be_bytes()).collect::<Vec<_>>();
        for a in &self.subs {
            bytes.append(&mut a.serialize());
        }
        bytes
    }
    fn print(&self, opt: &PrintOption) -> String {
        let pre = opt.indent.repeat(opt.tab);
        let num = self.subs.len();
        let mut buf = String::new();
        if opt.desc {
            if let Some(elements) = extract_packlist_elements(self.inst, &self.subs, opt) {
                return format!("{}[{}]", pre, elements.join(", "));
            }
            let mut prefix = String::new();
            if opt.tab == 0 && self.inst == Bytecode::IRBLOCK {
                if let Some(map) = opt.map {
                    for (idx, info) in map.lib_entries() {
                        let line = match &info.address {
                            Some(addr) => format!("lib {} = {}: {}\n", info.name, idx, addr.readable()),
                            None => format!("lib {} = {}:\n", info.name, idx),
                        };
                        prefix.push_str(&line);
                    }
                }
            }
            buf.push_str(&prefix);
            if opt.trim_root_block && opt.tab == 0 && self.inst == Bytecode::IRBLOCK {
                let mut start_idx = 0;
                if opt.trim_head_alloc {
                    if let Some(first) = self.subs.first() {
                        if first.bytecode() == Bytecode::ALLOC as u8 {
                            start_idx = 1;
                        }
                    }
                }
                let mut body_start_idx = start_idx;
                let mut param_line = None;
                if opt.trim_param_unpack {
                    if let Some(map) = opt.map {
                        if let Some(names) = map.param_names() {
                            if body_start_idx < num {
                                if let Some(double) = self.subs[body_start_idx].as_any().downcast_ref::<IRNodeDouble>() {
                                    if double.inst == Bytecode::UPLIST {
                                        let indent = opt.indent.repeat(opt.tab);
                                        let params = names.join(", ");
                                        param_line = Some(format!("{}param {{ {} }}", indent, params));
                                        body_start_idx += 1;
                                    }
                                }
                            }
                        }
                    }
                }
                let has_body = param_line.is_some() || body_start_idx < num;
                if has_body {
                    buf.push('\n');
                    if let Some(line) = param_line {
                        buf.push_str(&line);
                        buf.push('\n');
                    }
                    for a in &self.subs[body_start_idx..] {
                        buf.push_str(&a.print(opt));
                        buf.push('\n');
                    }
                    buf.pop();
                }
                return buf;
            }
            buf.push('{');
            if num > 0 {
                buf.push('\n');
                for a in &self.subs {
                    buf.push_str(&a.print(&opt.child()));
                    buf.push('\n');
                }
                buf.push_str(&pre);
            }
            buf.push('}');
            return buf;
        } else {
            buf.push_str(&format!("{}{:?} {} :\n", pre, self.inst, num));
        }
        if num == 0 {
            return buf
        }
        for a in &self.subs {
            buf.push_str(&a.print(opt));
            buf.push('\n');
        }
        buf.pop();
        buf
    }
}


#[allow(dead_code)]
impl $name {
    pub fn new() -> Self {
        Self {
            subs: vec![],
            inst: $inst,
        }
    }
    pub fn with_opcode(inst: Bytecode) -> Self {
        Self {
            subs: vec![],
            inst,
        }
    }
    pub fn with_capacity(n: usize) -> Ret<Self> {
        if n > u16::MAX as usize {
            return errf!("{} length max {}", stringify!($name), u16::MAX)
        }
        Ok(Self{
            subs: Vec::with_capacity(n),
            inst: $inst,
        })
    }
    pub fn from_vec(subs: Vec<Box<dyn IRNode>>) -> Ret<Self> {
        if subs.len() > u16::MAX as usize {
            return errf!("{} length max {}", stringify!($name), u16::MAX)
        }
        Ok(Self{subs, inst: $inst})
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


   
} }


define_ir_list_or_block!{ IRNodeList,  IRLIST,  compile_list }
define_ir_list_or_block!{ IRNodeBlock, IRBLOCK, compile_block }


/*******************************/




fn literals(s: String) -> String {
    s.replace("\\", "\\\\")
    .replace("\t", "\\t")
    .replace("\n", "\\n")
    .replace("\r", "\\r")
    .replace("\"", "\\\"")
}
