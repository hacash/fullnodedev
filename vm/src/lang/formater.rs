use field::Address as FieldAddress;
/// Handles descriptive decompilation and formatting logic.
#[derive(Clone)]
pub struct Formater<'a> {
    opt: PrintOption<'a>,
}

impl<'a> Formater<'a> {
    pub fn new(opt: &PrintOption<'a>) -> Self {
        Self { opt: opt.clone() }
    }

    fn child(&self) -> Self {
        Self { opt: self.opt.child() }
    }

    fn with_tab(&self, t: usize) -> Self {
        Self { opt: self.opt.with_tab(t) }
    }

    fn slot_name_display(&self, slot: u8) -> String {
        self.opt
            .map
            .and_then(|m| m.slot(slot))
            .cloned()
            .unwrap_or_else(|| format!("${}", slot))
    }

    fn literals(&self, s: String) -> String {
        s.replace("\\", "\\\\")
            .replace("\t", "\\t")
            .replace("\n", "\\n")
            .replace("\r", "\\r")
            .replace("\"", "\\\"")
    }

    fn extract_const_usize(&self, node: &dyn IRNode) -> Option<usize> {
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
                        if para.is_empty() { return None; }
                        let len = para[0] as usize;
                        if len > para.len().saturating_sub(1) { return None; }
                        if len == 0 { return Some(0); }
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

    fn extract_packlist_elements(&self, inst: Bytecode, subs: &[Box<dyn IRNode>]) -> Option<Vec<String>> {
        use Bytecode::*;
        if inst != IRLIST { return None; }
        let num = subs.len();
        if num < 2 { return None; }
        let last = &subs[num - 1];
        if let Some(leaf) = last.as_any().downcast_ref::<IRNodeLeaf>() {
            if leaf.inst != PACKLIST { return None; }
        } else { return None; }
        let count_idx = num - 2;
        let count = num - 2;
        let expected = self.extract_const_usize(&*subs[count_idx])?;
        if expected != count { return None; }
        let mut elems = Vec::with_capacity(count);
        for node in &subs[..count] {
            elems.push(self.print_inline(&**node));
        }
        Some(elems)
    }

    fn format_call_args(&self, substr: &str) -> String {
        let lines: Vec<String> = substr
            .lines()
            .map(|line| line.trim())
            .filter(|line| !line.is_empty())
            .map(|line| line.to_string())
            .collect();
        lines.join(", ")
    }

    fn collect_native_call_args(&self, node: &dyn IRNode) -> Vec<String> {
        use Bytecode::*;
        let mut args = Vec::new();
        let mut current: &dyn IRNode = node;
        loop {
            if let Some(list) = current.as_any().downcast_ref::<IRNodeArray>() {
                if let Some(elements) = self.extract_packlist_elements(list.inst, &list.subs) {
                    args.extend(elements);
                    return args;
                }
            }
            if let Some(double) = current.as_any().downcast_ref::<IRNodeDouble>() {
                if double.inst == CAT {
                    args.push(self.print_inline(&*double.subx));
                    current = &*double.suby;
                    continue;
                }
            }
            args.push(self.print_inline(current));
            break;
        }
        args
    }

    fn trim_nil_args(&self, args: &mut Vec<String>, node: &dyn IRNode) {
        use Bytecode::*;
        if self.opt.hide_func_nil_argv && node.bytecode() == PNIL as u8 {
            args.clear();
        }
    }

    fn build_call_args(&self, node: &dyn IRNode) -> String {
        let mut args_list = self.collect_native_call_args(node);
        self.trim_nil_args(&mut args_list, node);
        let args_src = args_list.join("\n");
        self.format_call_args(&args_src)
    }

    fn resolve_type_check_name(&self, ty: u8) -> Option<&'static str> {
        match ValueTy::build(ty) {
            Ok(vt) => Some(vt.name()),
            Err(_) => None,
        }
    }

    fn format_is_components(&self, node: &dyn IRNode) -> Option<(String, String)> {
        use Bytecode::*;
        if let Some(inner) = node.as_any().downcast_ref::<IRNodeSingle>() {
            let target = self.print_inline(&*inner.subx);
            return match inner.inst {
                TNIL => Some((target, "nil".to_string())),
                TLIST => Some((target, "list".to_string())),
                TMAP => Some((target, "map".to_string())),
                _ => None,
            };
        }
        if let Some(inner) = node.as_any().downcast_ref::<IRNodeParam1Single>() {
            if inner.inst == TIS {
                let target = self.print_inline(&*inner.subx);
                if let Some(name) = self.resolve_type_check_name(inner.para) {
                    return Some((target, name.to_string()));
                } else {
                    return Some((target, format!("{}", inner.para)));
                }
            }
        }
        None
    }

    fn print_param1_single(&self, node: &IRNodeParam1Single) -> String {
        use Bytecode::*;
        let pre = self.opt.indent.repeat(self.opt.tab);
        let meta = node.inst.metadata();
        let body = match node.inst {
            TIS => {
                let substr = self.print_inline(&*node.subx);
                match self.resolve_type_check_name(node.para) {
                    Some(name) => format!("{} is {}", substr, name),
                    None => format!("type_id({}) == {}", substr, node.para),
                }
            }
            PUT => {
                let substr = self.print_inline(&*node.subx);
                let slot_name = self.opt.map.and_then(|s| s.slot(node.para)).cloned();
                let is_first = self.opt.mark_slot_put(node.para);
                let target = if is_first {
                    let prefix = if self.opt.map.map(|m| m.slot_is_var(node.para)).unwrap_or(false) {
                        "var"
                    } else if self.opt.map.map(|m| m.slot_is_let(node.para)).unwrap_or(false) {
                        "let"
                    } else {
                        "var"
                    };
                    match slot_name {
                        Some(name) => format!("{} {} ${}", prefix, name, node.para),
                        None => format!("{} ${}", prefix, node.para),
                    }
                } else {
                    slot_name.unwrap_or_else(|| format!("${}", node.para))
                };
                format!("{} = {}", target, substr)
            }
            XOP => {
                let substr = self.print_inline(&*node.subx);
                let (op_str, idx) = local_operand_param_parse(node.para);
                let target = self.slot_name_display(idx);
                format!("{} {} {}", target, op_str, substr)
            }
            XLG => {
                let substr = self.print_inline(&*node.subx);
                let (opt_str, idx) = local_logic_param_parse(node.para);
                let target = self.slot_name_display(idx);
                format!("{} {} {}", target, opt_str, substr)
            }
            EXTFUNC => {
                let substr = self.print_inline(&*node.subx);
                let ary = CALL_EXTEND_FUNC_DEFS;
                let f = search_ext_name_by_id(node.para, &ary);
                format!("{}({})", f, substr)
            }
            EXTACTION => {
                let args = self.build_call_args(&*node.subx);
                let ary = CALL_EXTEND_ACTION_DEFS;
                let f = search_ext_name_by_id(node.para, &ary);
                format!("{}({})", f, args)
            }
            NTCALL => {
                let args = self.build_call_args(&*node.subx);
                let ntcall: NativeCall = std_mem_transmute!(node.para);
                format!("{}({})", ntcall.name(), args)
            }
            _ => {
                let substr = self.print_sub(&*node.subx);
                format!("{}({}, {})", meta.intro, node.para, substr)
            }
        };
        format!("{}{}", pre, body)
    }

    fn format_leaf(&self, leaf: &IRNodeLeaf) -> String {
        use Bytecode::*;
        let mut buf = self.opt.indent.repeat(self.opt.tab);
        let meta = leaf.inst.metadata();
        match leaf.inst {
            NOP => {}
            GET3 => buf.push_str(&self.slot_name_display(3)),
            GET2 => buf.push_str(&self.slot_name_display(2)),
            GET1 => buf.push_str(&self.slot_name_display(1)),
            GET0 => buf.push_str(&self.slot_name_display(0)),
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
        buf
    }

    fn format_param1(&self, node: &IRNodeParam1) -> String {
        use Bytecode::*;
        let mut buf = self.opt.indent.repeat(self.opt.tab);
        let meta = node.inst.metadata();
        match node.inst {
            PU8 => buf.push_str(&format!("{}", node.para)),
            GET => buf.push_str(&format!("${}", node.para)),
            EXTENV => {
                let ary = CALL_EXTEND_ENV_DEFS;
                let f = search_ext_name_by_id(node.para, &ary);
                buf.push_str(&format!("{}()", f));
            }
            _ => {
                buf.push_str(&format!("{}({})", meta.intro, node.para));
            }
        };
        buf
    }

    fn format_param2(&self, node: &IRNodeParam2) -> String {
        use Bytecode::*;
        let mut buf = self.opt.indent.repeat(self.opt.tab);
        let meta = node.inst.metadata();
        match node.inst {
            PU16 => buf.push_str(&format!("{}", u16::from_be_bytes(node.para))),
            _ => {
                let para = hex::encode(node.para);
                buf.push_str(&format!("{}(0x{})", meta.intro, para));
            }
        };
        buf
    }

    fn format_data_bytes(&self, node: &IRNodeParams) -> String {
        use Bytecode::*;
        let l = if node.inst == PBUF { 1usize } else { 2usize };
        let data = node.para[l..].to_vec();
        if let Some(s) = self.ascii_show_string(&data) {
            return format!("\"{}\"", self.literals(s));
        }
        if data.len() == FieldAddress::SIZE {
            let addr = FieldAddress::must_vec(data.clone());
            if addr.check_version().is_ok() {
                return addr.readable();
            }
        }
        format!("0x{}", hex::encode(&data))
    }

    fn format_params(&self, node: &IRNodeParams) -> String {
        use Bytecode::*;
        let mut buf = self.opt.indent.repeat(self.opt.tab);
        let meta = node.inst.metadata();
        let parastr = hex::encode(&node.para);
        match node.inst {
            PBUF | PBUFL => {
                buf.push_str(&self.format_data_bytes(node));
            }
            CALLCODE => {
                let i = node.para[0];
                let f = ::hex::encode(&node.para[1..]);
                buf.push_str(&format!("callcode {}::{}", i, f));
            }
            _ => {
                buf.push_str(&format!("{}(0x{})", meta.intro, parastr));
            }
        }
        buf
    }

    fn print_descriptive(&self, node: &dyn IRNode) -> String {
        if let Some(leaf) = node.as_any().downcast_ref::<IRNodeLeaf>() {
            return self.format_leaf(leaf);
        }
        if let Some(bytecodes) = node.as_any().downcast_ref::<IRNodeBytecodes>() {
            let buf = self.opt.indent.repeat(self.opt.tab);
            let codes = bytecodes.codes.bytecode_print(false).unwrap();
            let codes = codes.trim_end();
            return format!("{}bytecode {{ {} }}", buf, codes);
        }
        if let Some(param1) = node.as_any().downcast_ref::<IRNodeParam1>() {
            return self.format_param1(param1);
        }
        if let Some(param2) = node.as_any().downcast_ref::<IRNodeParam2>() {
            return self.format_param2(param2);
        }
        if let Some(params) = node.as_any().downcast_ref::<IRNodeParams>() {
            return self.format_params(params);
        }
        self.format_instruction_preview(node)
    }

    fn format_instruction_preview(&self, node: &dyn IRNode) -> String {
        let code: Bytecode = std_mem_transmute!(node.bytecode());
        let meta = code.metadata();
        let prefix = self.opt.indent.repeat(self.opt.tab);
        if let Some(single) = node.as_any().downcast_ref::<IRNodeSingle>() {
            let child = self.print_inline(&*single.subx);
            return format!("{}{}({})", prefix, meta.intro, child);
        }
        if let Some(double) = node.as_any().downcast_ref::<IRNodeDouble>() {
            let subx = self.print_inline(&*double.subx);
            let suby = self.print_inline(&*double.suby);
            return format!("{}{}({}, {})", prefix, meta.intro, subx, suby);
        }
        if let Some(triple) = node.as_any().downcast_ref::<IRNodeTriple>() {
            let subx = self.print_inline(&*triple.subx);
            let suby = self.print_inline(&*triple.suby);
            let subz = self.print_inline(&*triple.subz);
            return format!(
                "{}{}({}, {}, {})",
                prefix, meta.intro, subx, suby, subz
            );
        }
        format!("{}{}", prefix, meta.intro)
    }

    /// Main entry for descriptive printing.
    pub fn print(&self, node: &dyn IRNode) -> String {
        self.print_inner(node)
    }

    fn print_inner(&self, node: &dyn IRNode) -> String {
        use Bytecode::*;
        if let Some(pss) = node.as_any().downcast_ref::<IRNodeParam1Single>() {
            return self.print_param1_single(pss);
        }
        let code: Bytecode = std_mem_transmute!(node.bytecode());
        match code {
            MPUT => {
                if let Some(double) = node.as_any().downcast_ref::<IRNodeDouble>() {
                    let key = self.print_inline(&*double.subx);
                    let value = self.print_inline(&*double.suby);
                    let meta = MPUT.metadata();
                    return format!(
                        "{}{}({}, {})",
                        self.opt.indent.repeat(self.opt.tab),
                        meta.intro,
                        key,
                        value
                    );
                }
                return self.print_descriptive(node);
            }
            IRLIST | IRBLOCK | IRBLOCKR => {
                if let Some(arr) = node.as_any().downcast_ref::<IRNodeArray>() {
                    if arr.inst == IRLIST {
                        if let Some(elements) = self.extract_packlist_elements(arr.inst, &arr.subs) {
                            return format!("{}[{}]", self.opt.indent.repeat(self.opt.tab), elements.join(", "));
                        }
                    }
                    let mut prefix = String::new();
                    if self.opt.tab == 0 && arr.inst == IRBLOCK {
                        if let Some(map) = self.opt.map {
                            for (idx, info) in map.lib_entries() {
                                let line = match &info.address {
                                    Some(addr) => format!("lib {} = {}: {}\n", info.name, idx, addr.readable()),
                                    None => format!("lib {} = {}:\n", info.name, idx),
                                };
                                prefix.push_str(&line);
                            }
                        }
                    }
                    let mut buf = prefix;
                    if self.opt.trim_root_block && self.opt.tab == 0 && arr.inst == IRBLOCK {
                        let mut start_idx = 0;
                        if self.opt.trim_head_alloc {
                            if let Some(first) = arr.subs.first() {
                                if first.bytecode() == ALLOC as u8 {
                                    start_idx = 1;
                                }
                            }
                        }
                        let mut body_start_idx = start_idx;
                        let mut param_line = None;
                        if self.opt.trim_param_unpack {
                            if let Some(map) = self.opt.map {
                                if let Some(names) = map.param_names() {
                                    if body_start_idx < arr.subs.len() {
                                        if let Some(double) = arr.subs[body_start_idx].as_any().downcast_ref::<IRNodeDouble>() {
                                            if double.inst == UPLIST {
                                                let indent = self.opt.indent.repeat(self.opt.tab);
                                                let params = names.join(", ");
                                                param_line = Some(format!("{}param {{ {} }}", indent, params));
                                                body_start_idx += 1;
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        let has_body = param_line.is_some() || body_start_idx < arr.subs.len();
                        if has_body {
                            buf.push('\n');
                            if let Some(line) = param_line {
                                buf.push_str(&line);
                                buf.push('\n');
                            }
                            for a in &arr.subs[body_start_idx..] {
                                buf.push_str(&self.child().print(&**a));
                                buf.push('\n');
                            }
                            if buf.ends_with('\n') { buf.pop(); }
                            return buf;
                        }
                    }
                    buf.push('{');
                    if !arr.subs.is_empty() {
                        buf.push('\n');
                        for a in &arr.subs {
                            buf.push_str(&self.child().print(&**a));
                            buf.push('\n');
                        }
                        buf.push_str(&self.opt.indent.repeat(self.opt.tab));
                    }
                    buf.push('}');
                    return buf;
                }else{
                    panic!("IRNodeArray expected for IRLIST/IRBLOCK/IRBLOCKR")
                }
            }

            _ if OpTy::from_bytecode(code).is_ok() => {
                if code == NOT {
                    if let Some(s) = node.as_any().downcast_ref::<IRNodeSingle>() {
                        let pre = self.opt.indent.repeat(self.opt.tab);
                        let substr = self.print_inline(&*s.subx);
                        if let Some((target, ty)) = self.format_is_components(&*s.subx) {
                            return format!("{}{} is not {}", pre, target, ty);
                        } else {
                            return format!("{}! {}", pre, substr);
                        }
                    }
                }
                if let Some(d) = node.as_any().downcast_ref::<IRNodeDouble>() {
                    let sg = OpTy::from_bytecode(d.inst).unwrap().symbol();
                    let res = self.print_subx_suby_op(d, sg);
                    let pre = self.opt.indent.repeat(self.opt.tab);
                    return format!("{}{}", pre, res);
                }
                return self.print_descriptive(node)
            }

            PBUF | PBUFL | CALLCODE => {
                if let Some(p) = node.as_any().downcast_ref::<IRNodeParams>() {
                    let pre = self.opt.indent.repeat(self.opt.tab);
                    match p.inst {
                        PBUF | PBUFL => {
                            let l = if p.inst == PBUF { 1usize } else { 2usize };
                            let data = p.para[l..].to_vec();
                            if let Some(s) = self.ascii_show_string(&data) {
                                return format!("{}\"{}\"", pre, self.literals(s));
                            }
                            if data.len() == FieldAddress::SIZE {
                                let addr = FieldAddress::must_vec(data.clone());
                                if let Ok(..) = addr.check_version() {
                                    return format!("{}{}", pre, addr.readable());
                                }
                            }
                            return format!("{}0x{}", pre, ::hex::encode(&data));
                        }
                        CALLCODE => {
                            let i = p.para[0];
                            let f = ::hex::encode(&p.para[1..]);
                            return format!("{}callcode {}::{}", self.opt.indent.repeat(self.opt.tab), i, f);
                        }
                        _ => {}
                    }
                    return self.print_descriptive(node)
                } else { 
                    return self.print_descriptive(node)
                }
            }

            CALL | CALLLIB | CALLINR | CALLSTATIC => {
                if let Some(pss) = node.as_any().downcast_ref::<IRNodeParamsSingle>() {
                    let pre = self.opt.indent.repeat(self.opt.tab);
                    let args = self.build_call_args(&*pss.subx);
                    let mut buf = pre;
                    let meta = pss.inst.metadata();
                    match pss.inst {
                        CALL => {
                            if let Some((lib, func)) = (|| {
                                if pss.para.len() >= 5 {
                                    let idx = pss.para[0];
                                    let mut sig = [0u8;4];
                                    sig.copy_from_slice(&pss.para[1..5]);
                                    if let Some(map) = self.opt.map {
                                        if let Some(libinfo) = map.lib(idx) {
                                            if let Some(fname) = map.func(&sig) {
                                                return Some((libinfo.name.clone(), fname.clone()));
                                            }
                                        }
                                    }
                                }
                                None
                            })() {
                                buf.push_str(&format!("{}.{}({})", lib, func, args));
                            } else {
                                let idx = pss.para[0];
                                let f = ::hex::encode(&pss.para[1..]);
                                buf.push_str(&format!("call {}::{}({})", idx, f, args));
                            }
                            return buf;
                        }
                        CALLLIB => {
                            if let Some((lib, func)) = (|| {
                                if pss.para.len() >= 5 {
                                    let idx = pss.para[0];
                                    let mut sig = [0u8;4];
                                    sig.copy_from_slice(&pss.para[1..5]);
                                    if let Some(map) = self.opt.map {
                                        if let Some(libinfo) = map.lib(idx) {
                                            if let Some(fname) = map.func(&sig) {
                                                return Some((libinfo.name.clone(), fname.clone()));
                                            }
                                        }
                                    }
                                }
                                None
                            })() {
                                buf.push_str(&format!("{}:{}({})", lib, func, args));
                            } else {
                                let idx = pss.para[0];
                                let f = ::hex::encode(&pss.para[1..]);
                                buf.push_str(&format!("calllib {}:{}({})", idx, f, args));
                            }
                            return buf;
                        }
                        CALLINR => {
                            if let Some(func) = (|| {
                                if pss.para.len() == 4 {
                                    let mut sig = [0u8;4];
                                    sig.copy_from_slice(&pss.para);
                                    if let Some(map) = self.opt.map { return map.func(&sig).cloned(); }
                                }
                                None
                            })() {
                                buf.push_str(&format!("self.{}({})", func, args));
                            } else {
                                let f = ::hex::encode(&pss.para);
                                buf.push_str(&format!("callinr {}({})", f, args));
                            }
                            return buf;
                        }
                        CALLSTATIC => {
                            if let Some((lib, func)) = (|| {
                                if pss.para.len() >= 5 {
                                    let idx = pss.para[0];
                                    let mut sig = [0u8;4];
                                    sig.copy_from_slice(&pss.para[1..5]);
                                    if let Some(map) = self.opt.map {
                                        if let Some(libinfo) = map.lib(idx) {
                                            if let Some(fname) = map.func(&sig) {
                                                return Some((libinfo.name.clone(), fname.clone()));
                                            }
                                        }
                                    }
                                }
                                None
                            })() {
                                buf.push_str(&format!("{}::{}({})", lib, func, args));
                            } else {
                                let idx = pss.para[0];
                                let f = ::hex::encode(&pss.para[1..]);
                                buf.push_str(&format!("callstatic {}::{}({})", idx, f, args));
                            }
                            return buf;
                        }
                        _ => {
                            buf.push_str(&format!("{}({})", meta.intro, args));
                            return buf;
                        }
                    }
                }
                return self.format_instruction_preview(node);
            }

            _ => {
                // Unary/triple and other detailed cases
                if let Some(s) = node.as_any().downcast_ref::<IRNodeSingle>() {
                    let pre = self.opt.indent.repeat(self.opt.tab);
                    match s.inst {
                        TNIL | TLIST | TMAP => {
                            let substr = self.print_inline(&*s.subx);
                            return match s.inst {
                                TNIL => format!("{}{} is nil", pre, substr),
                                TLIST => format!("{}{} is list", pre, substr),
                                TMAP  => format!("{}{} is map", pre, substr),
                                _ => unreachable!(),
                            };
                        }
                        CU8 | CU16 | CU32 | CU64 | CU128 |
                        RET | ERR | AST => {
                            let substr = self.print_inline(&*s.subx);
                            let operand = if s.subx.level() > 0 {
                                let t = substr.trim();
                                if t.starts_with('(') && t.ends_with(')') { substr.clone() } else { format!("({})", substr) }
                            } else { substr.clone() };
                            match s.inst {
                                CU8  => return format!("{}{} as u8", pre, operand),
                                CU16 => return format!("{}{} as u16", pre, operand),
                                CU32 => return format!("{}{} as u32", pre, operand),
                                CU64 => return format!("{}{} as u64", pre, operand),
                                CU128 => return format!("{}{} as u128", pre, operand),
                                RET | ERR | AST => {
                                    let meta = s.inst.metadata();
                                    return format!("{}{} {}", pre, meta.intro, substr);
                                }
                                _ => {}
                            }
                        }
                        PRT => {
                            let substr = self.print_inline(&*s.subx);
                            let meta = s.inst.metadata();
                            return format!("{}{} {}", self.opt.indent.repeat(self.opt.tab), meta.intro, substr);
                        }
                        MGET => {
                            let substr = self.print_inline(&*s.subx);
                            let meta = s.inst.metadata();
                            return format!("{}{}({})", pre, meta.intro, substr);
                        }
                        _ => {}
                    }
                }

                if let Some(d) = node.as_any().downcast_ref::<IRNodeDouble>() {
                    if d.inst == ITEMGET {
                        let subxstr = self.print_sub(&*d.subx);
                        let subystr = self.print_inline(&*d.suby);
                        return format!("{}{}[{}]", self.opt.indent.repeat(self.opt.tab), subxstr, subystr);
                    }
                }

                if let Some(t) = node.as_any().downcast_ref::<IRNodeTriple>() {
                    if t.inst == IRIF || t.inst == IRIFR {
                        let subxstr = self.print_inline(&*t.subx);
                        let subystr = self.print_newline(&*t.suby);
                        let subzstr = self.print_newline(&*t.subz);
                        let mut buf = format!("{}if {} {{{}}}", self.opt.indent.repeat(self.opt.tab), subxstr, subystr);
                        if subzstr.len() > 0 {
                            buf.push_str(&format!(" else {{{}}}", subzstr));
                        }
                        return buf;
                    }
                }
                return self.print_descriptive(node)
            }
            // _ => return self.print(node),
        }
    }

    /// Inline printing (equivalent to `print_sub_inline`).
    pub fn print_inline(&self, node: &dyn IRNode) -> String {
        let inline = self.with_tab(0);
        let substr = inline.print(node);
        if substr.contains('\n') {
            let t = substr.trim();
            if t.starts_with('{') && t.ends_with('}') && t.len() >= 2 {
                return t[1..t.len()-1].trim().to_owned();
            }
            return t.replace('\n', " ");
        }
        substr
    }

    pub fn print_sub(&self, node: &dyn IRNode) -> String {
        if node.subs() == 0 {
            return self.print_descriptive(node);
        }
        let child = self.child();
        let mut buf = String::from("\n") + &child.print(node);
        buf += &(String::from("\n") + &self.opt.indent.repeat(self.opt.tab));
        buf
    }

    pub fn print_newline(&self, node: &dyn IRNode) -> String {
        let child = self.child();
        let sub = child.print(node);
        let emp = sub.replace(" ", "").replace("\n", "");
        if emp.len() > 0 {
            let mut buf = String::from("\n") + &sub;
            buf += &(String::from("\n") + &self.opt.indent.repeat(self.opt.tab));
            buf
        } else { emp }
    }

    pub fn print_subx_suby_op(&self, dbl: &IRNodeDouble, op: &str) -> String {
        let inline_opt = self.with_tab(0);
        let mut subx = inline_opt.print_inline(&*dbl.subx);
        let mut suby = inline_opt.print_inline(&*dbl.suby);
        let wrapx = dbl.subx.as_any().downcast_ref::<IRNodeWrapOne>().is_some();
        let wrapy = dbl.suby.as_any().downcast_ref::<IRNodeWrapOne>().is_some();
        let clv = match OpTy::from_bytecode(dbl.inst) { Ok(t) => t.level(), _ => 0 };
        let llv = dbl.subx.level();
        let rlv = dbl.suby.level();
        if clv>0 && llv>0 && clv>llv && !wrapx { subx = format!("({})", &subx); }
        let need_wrap_right = clv>0 && rlv>0 && !wrapy && (clv>rlv || clv==rlv);
        if need_wrap_right { suby = format!("({})", &suby); }
        format!("{} {} {}", subx, op, suby)
    }

    // Added: local `ascii_show_string` implementation to avoid relying on external imports.
    fn ascii_show_string(&self, data: &[u8]) -> Option<String> {
        if data.is_empty() { return Some(String::new()); }
        // Determine printable ASCII (common newline/tab are treated as non-printable by the caller).
        if data.iter().all(|&b| b >= 0x20 && b <= 0x7E) {
            match std::str::from_utf8(data) {
                Ok(s) => Some(s.to_string()),
                Err(_) => None,
            }
        } else {
            None
        }
    }

}
