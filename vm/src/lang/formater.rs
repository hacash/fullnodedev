use field::Address as FieldAddress;
/// Handles descriptive decompilation and formatting logic.
#[derive(Clone)]
pub struct Formater<'a> {
    opt: PrintOption<'a>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum LiteralKind {
    Numeric,
    Text,
    Address,
}

#[derive(Clone)]
struct RecoveredLiteral {
    text: String,
    kind: LiteralKind,
}

impl RecoveredLiteral {
    fn numeric<S: Into<String>>(value: S) -> Self {
        Self {
            text: value.into(),
            kind: LiteralKind::Numeric,
        }
    }

    fn text<S: Into<String>>(value: S) -> Self {
        Self {
            text: value.into(),
            kind: LiteralKind::Text,
        }
    }

    fn address<S: Into<String>>(value: S) -> Self {
        Self {
            text: value.into(),
            kind: LiteralKind::Address,
        }
    }
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

    fn resolve_lib_func(&self, idx: u8, sig: &[u8]) -> Option<(String, String)> {
        if sig.len() < 4 {
            return None;
        }
        let map = self.opt.map?;
        let libinfo = map.lib(idx)?;
        let mut key = [0u8; 4];
        key.copy_from_slice(&sig[0..4]);
        let func = map.func(&key)?;
        Some((libinfo.name.clone(), func.clone()))
    }

    fn resolve_func_name(&self, sig: &[u8]) -> Option<String> {
        if sig.len() != 4 {
            return None;
        }
        let map = self.opt.map?;
        let mut key = [0u8; 4];
        key.copy_from_slice(sig);
        map.func(&key).cloned()
    }

    fn short_call_target(
        &self,
        code: Bytecode,
        pss: &IRNodeParamsSingle,
        args: &str,
    ) -> Option<String> {
        use Bytecode::*;
        match code {
            CALLINR => self
                .resolve_func_name(&pss.para)
                .map(|func| format!("self.{}({})", func, args)),
            CALL => self
                .resolve_lib_func(pss.para[0], &pss.para[1..])
                .map(|(lib, func)| format!("{}.{}({})", lib, func, args)),
            CALLLIB => self
                .resolve_lib_func(pss.para[0], &pss.para[1..])
                .map(|(lib, func)| format!("{}:{}({})", lib, func, args)),
            CALLSTATIC => self
                .resolve_lib_func(pss.para[0], &pss.para[1..])
                .map(|(lib, func)| format!("{}::{}({})", lib, func, args)),
            _ => None,
        }
    }

    fn literals(&self, s: String) -> String {
        s.replace("\\", "\\\\")
            .replace("\t", "\\t")
            .replace("\n", "\\n")
            .replace("\r", "\\r")
            .replace("\"", "\\\"")
    }

    fn line_prefix(&self) -> String {
        self.opt.indent.repeat(self.opt.tab)
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

    fn collect_native_call_args(&self, node: &dyn IRNode, system_call: bool) -> Vec<String> {
        use Bytecode::*;
        let mut args = Vec::new();
        let mut current: &dyn IRNode = node;
        let helper = DecompilationHelper::new(&self.opt);
        loop {
            if self.opt.flatten_call_packlist {
                if let Some(list) = current.as_any().downcast_ref::<IRNodeArray>() {
                    if let Some(elements) = self.extract_packlist_elements(list.inst, &list.subs) {
                        args.extend(elements);
                        return args;
                    }
                }
            }
            if let Some(double) = current.as_any().downcast_ref::<IRNodeDouble>() {
                if double.inst == CAT && (!system_call || helper.should_flatten_syscall_cat()) {
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

    fn build_call_args(&self, node: &dyn IRNode, system_call: bool) -> String {
        let mut args_list = self.collect_native_call_args(node, system_call);
        self.trim_nil_args(&mut args_list, node);
        let args_src = args_list.join("\n");
        self.format_call_args(&args_src)
    }

    fn format_memory_put(&self, node: &dyn IRNode) -> Option<String> {
        use Bytecode::*;
        let double = node.as_any().downcast_ref::<IRNodeDouble>()?;
        let code: Bytecode = std_mem_transmute!(node.bytecode());
        if code != MPUT {
            return None;
        }
        let key = self.print_inline(&*double.subx);
        let value = self.print_inline(&*double.suby);
        let meta = MPUT.metadata();
        Some(format!(
            "{}{}({}, {})",
            self.line_prefix(),
            meta.intro,
            key,
            value
        ))
    }

    fn format_array_block(&self, node: &dyn IRNode) -> Option<String> {
        use Bytecode::*;
        let arr = node.as_any().downcast_ref::<IRNodeArray>()?;
        let helper = DecompilationHelper::new(&self.opt);
        if arr.inst != IRLIST && arr.inst != IRBLOCK && arr.inst != IRBLOCKR {
            return None;
        }
        let prefix = self.line_prefix();
        if arr.inst == IRLIST {
            if let Some(elements) = self.extract_packlist_elements(arr.inst, &arr.subs) {
                if self.opt.flatten_array_packlist {
                    return Some(format!("{}[{}]", prefix, elements.join(", ")));
                } else {
                    let args = elements.join(", ");
                    let mut buf = format!("{}packlist {{", prefix);
                    if !args.is_empty() {
                        buf.push(' ');
                        buf.push_str(&args);
                        buf.push(' ');
                    }
                    buf.push('}');
                    return Some(buf);
                }
            }
            if let Some(last) = arr.subs.last() {
                let code: Bytecode = std_mem_transmute!(last.bytecode());
                if matches!(code, LOG1 | LOG2 | LOG3 | LOG4) {
                    let args: Vec<String> = arr.subs[0..arr.subs.len()-1].iter()
                        .map(|s| self.print_inline(&**s))
                        .collect();
                    return Some(format!("{}log({})", prefix, args.join(", ")));
                }
            }
        }
        let mut buf = helper.block_prefix(arr);
        if helper.should_trim_root_block(arr) {
            let (body_start_idx, param_line) = helper.prepare_root_block(arr);
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
                if buf.ends_with('\n') {
                    buf.pop();
                }
                return Some(buf);
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
        Some(buf)
    }

    fn format_call_instruction(&self, node: &dyn IRNode, code: Bytecode) -> Option<String> {
        use Bytecode::*;
        let pss = node.as_any().downcast_ref::<IRNodeParamsSingle>()?;
        if !matches!(code, CALL | CALLLIB | CALLINR | CALLSTATIC) {
            return None;
        }
        let pre = self.line_prefix();
        let args = self.build_call_args(&*pss.subx, false);
        let meta = pss.inst.metadata();

        let default_body = match code {
            CALL => format!("call {}::0x{}({})", pss.para[0], ::hex::encode(&pss.para[1..]), args),
            CALLLIB => format!("calllib {}::0x{}({})", pss.para[0], ::hex::encode(&pss.para[1..]), args),
            CALLINR => format!("callinr 0x{}({})", ::hex::encode(&pss.para), args),
            CALLSTATIC => {
                format!("callstatic {}::0x{}({})", pss.para[0], ::hex::encode(&pss.para[1..]), args)
            }
            _ => format!("{}({})", meta.intro, args),
        };

        let short_body = if self.opt.call_short_syntax {
            self.short_call_target(code, pss, &args)
        } else {
            None
        };

        if let Some(short) = short_body {
            return Some(format!("{} /*{}*/ {}", pre, meta.intro, short));
        }

        Some(format!("{}{}", pre, default_body))
    }

    fn format_opty_double(&self, code: Bytecode, node: &dyn IRNode) -> Option<String> {
        if OpTy::from_bytecode(code).is_err() {
            return None;
        }
        let d = node.as_any().downcast_ref::<IRNodeDouble>()?;
        let sg = OpTy::from_bytecode(d.inst).unwrap().symbol();
        let res = self.print_subx_suby_op(d, sg);
        Some(format!("{}{}", self.line_prefix(), res))
    }

    fn format_unary_triple(&self, node: &dyn IRNode) -> Option<String> {
        use Bytecode::*;
        if let Some(s) = node.as_any().downcast_ref::<IRNodeSingle>() {
            let pre = self.line_prefix();
            match s.inst {
                TNIL | TLIST | TMAP => {
                    let substr = self.print_inline(&*s.subx);
                    return Some(match s.inst {
                        TNIL => format!("{}{} is nil", pre, substr),
                        TLIST => format!("{}{} is list", pre, substr),
                        TMAP => format!("{}{} is map", pre, substr),
                        _ => unreachable!(),
                    });
                }
                CU8 | CU16 | CU32 | CU64 | CU128 | RET | ERR | AST => {
                    let literal = self.literal_from_node(&*s.subx);
                    let substr = if let Some(ref lit) = literal {
                        lit.text.clone()
                    } else {
                        self.print_inline(&*s.subx)
                    };
                    let operand = if s.subx.level() > 0 {
                        let t = substr.trim();
                        if t.starts_with('(') && t.ends_with(')') {
                            substr.clone()
                        } else {
                            format!("({})", substr)
                        }
                    } else {
                        substr.clone()
                    };
                    let use_literal = literal
                        .as_ref()
                        .map(|lit| lit.kind == LiteralKind::Numeric)
                        .unwrap_or(false);
                    return Some(match s.inst {
                        CU8 => {
                            if use_literal {
                                format!("{}{}", pre, substr.trim())
                            } else {
                                format!("{}{} as u8", pre, operand)
                            }
                        }
                        CU16 => {
                            if use_literal {
                                format!("{}{}", pre, substr.trim())
                            } else {
                                format!("{}{} as u16", pre, operand)
                            }
                        }
                        CU32 => {
                            if use_literal {
                                format!("{}{}", pre, substr.trim())
                            } else {
                                format!("{}{} as u32", pre, operand)
                            }
                        }
                        CU64 => {
                            if use_literal {
                                format!("{}{}", pre, substr.trim())
                            } else {
                                format!("{}{} as u64", pre, operand)
                            }
                        }
                        CU128 => {
                            if use_literal {
                                format!("{}{}", pre, substr.trim())
                            } else {
                                format!("{}{} as u128", pre, operand)
                            }
                        }
                        RET | ERR | AST => {
                            let meta = s.inst.metadata();
                            format!("{}{} {}", pre, meta.intro, substr)
                        }
                        _ => unreachable!(),
                    });
                }
                NOT => {
                    let substr = self.print_inline(&*s.subx);
                    if let Some((target, ty)) = self.format_is_components(&*s.subx) {
                        return Some(format!("{}{} is not {}", pre, target, ty));
                    }
                    return Some(format!("{}! {}", pre, substr));
                }
                PRT => {
                    let substr = self.print_inline(&*s.subx);
                    let meta = s.inst.metadata();
                    return Some(format!("{}{} {}", pre, meta.intro, substr));
                }
                MGET => {
                    let substr = self.print_inline(&*s.subx);
                    let meta = s.inst.metadata();
                    return Some(format!("{}{}({})", pre, meta.intro, substr));
                }
                _ => {}
            }
        }
        if let Some(d) = node.as_any().downcast_ref::<IRNodeDouble>() {
            if d.inst == ITEMGET {
                let subxstr = self.print_sub(&*d.subx);
                let subystr = self.print_inline(&*d.suby);
                return Some(format!(
                    "{}{}[{}]",
                    self.line_prefix(),
                    subxstr,
                    subystr
                ));
            }
            if d.inst == IRWHILE {
                let subxstr = self.print_inline(&*d.subx);
                let subystr = self.print_newline(&*d.suby);
                return Some(format!(
                    "{}while {} {{{}}}",
                    self.line_prefix(),
                    subxstr,
                    subystr
                ));
            }
        }
        if let Some(t) = node.as_any().downcast_ref::<IRNodeTriple>() {
            if t.inst == IRIF || t.inst == IRIFR {
                let subxstr = self.print_inline(&*t.subx);
                let subystr = self.print_newline(&*t.suby);
                let subzstr = self.print_newline(&*t.subz);
                let mut buf = format!(
                    "{}if {} {{{}}}",
                    self.line_prefix(),
                    subxstr,
                    subystr
                );
                if subzstr.len() > 0 {
                    buf.push_str(&format!(" else {{{}}}", subzstr));
                }
                return Some(buf);
            }
        }
        None
    }

    fn literal_from_node(&self, node: &dyn IRNode) -> Option<RecoveredLiteral> {
        if !self.opt.recover_literals {
            return None;
        }
        use Bytecode::*;
        if let Some(leaf) = node.as_any().downcast_ref::<IRNodeLeaf>() {
            let literal = match leaf.inst {
                P0 => "0",
                P1 => "1",
                P2 => "2",
                P3 => "3",
                _ => return None,
            };
            return Some(RecoveredLiteral::numeric(literal));
        }
        if let Some(param1) = node.as_any().downcast_ref::<IRNodeParam1>() {
            if param1.inst == PU8 {
                return Some(RecoveredLiteral::numeric(param1.para.to_string()));
            }
        }
        if let Some(param2) = node.as_any().downcast_ref::<IRNodeParam2>() {
            if param2.inst == PU16 {
                return Some(RecoveredLiteral::numeric(
                    u16::from_be_bytes(param2.para).to_string(),
                ));
            }
        }
        if let Some(params) = node.as_any().downcast_ref::<IRNodeParams>() {
            if let Some(bytes) = self.params_to_bytes(params) {
                if let Some(literal) = self.decode_bytes_literal(bytes) {
                    return Some(literal);
                }
                if let Some(value) = self.bytes_to_u128(bytes) {
                    return Some(RecoveredLiteral::numeric(value.to_string()));
                }
            }
        }
        if let Some(single) = node.as_any().downcast_ref::<IRNodeParam1Single>() {
            if single.inst == CTO && single.para == ValueTy::Address as u8 {
                return self.literal_from_node(&*single.subx);
            }
        }
        if let Some(single) = node.as_any().downcast_ref::<IRNodeSingle>() {
            if single.inst == CBUF {
                return self.literal_from_node(&*single.subx);
            }
        }
        None
    }

    fn decode_bytes_literal(&self, data: &[u8]) -> Option<RecoveredLiteral> {
        if let Some(text) = self.ascii_show_string(data) {
            return Some(RecoveredLiteral::text(format!(
                "\"{}\"",
                self.literals(text)
            )));
        }
        if data.len() == FieldAddress::SIZE {
            let addr = FieldAddress::must_vec(data.to_vec());
            if addr.check_version().is_ok() {
                return Some(RecoveredLiteral::address(addr.readable()));
            }
        }
        None
    }

    fn params_to_bytes<'b>(&self, params: &'b IRNodeParams) -> Option<&'b [u8]> {
        use Bytecode::*;
        let header_len = match params.inst {
            PBUF => 1,
            PBUFL => 2,
            _ => return None,
        };
        if params.para.len() < header_len {
            return None;
        }
        let data_len = match header_len {
            1 => params.para[0] as usize,
            2 => {
                let hi = params.para[0];
                let lo = params.para[1];
                u16::from_be_bytes([hi, lo]) as usize
            }
            _ => unreachable!(),
        };
        let start = header_len;
        let end = start + data_len;
        if params.para.len() < end {
            return None;
        }
        Some(&params.para[start..end])
    }

    fn bytes_to_u128(&self, data: &[u8]) -> Option<u128> {
        if data.len() > 16 {
            return None;
        }
        let mut value = 0u128;
        for &b in data {
            value = (value << 8) | b as u128;
        }
        Some(value)
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
            XOP => self.format_local_param(node, local_operand_param_parse),
            XLG => self.format_local_param(node, local_logic_param_parse),
            EXTFUNC => self.format_extend_call(node, &CALL_EXTEND_FUNC_DEFS, true),
            EXTACTION => self.format_extend_call(node, &CALL_EXTEND_ACTION_DEFS, false),
            NTCALL => {
                let args = self.build_call_args(&*node.subx, true);
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

    fn format_extend_call(
        &self,
        node: &IRNodeParam1Single,
        defs: &[(u8, &'static str, ValueTy)],
        inline_arg: bool,
    ) -> String {
        let args = if inline_arg {
            self.print_inline(&*node.subx)
        } else {
            self.build_call_args(&*node.subx, false)
        };
        let f = search_ext_name_by_id(node.para, defs);
        format!("{}({})", f, args)
    }

    fn format_local_param<F>(&self, node: &IRNodeParam1Single, parser: F) -> String
    where
        F: Fn(u8) -> (String, u8),
    {
        let substr = self.print_inline(&*node.subx);
        let (op_str, idx) = parser(node.para);
        let target = self.slot_name_display(idx);
        format!("{} {} {}", target, op_str, substr)
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
            PNIL => buf.push_str("nil"),
            PNBUF => buf.push_str("\"\""),
            NEWLIST => buf.push_str("[]"),
            ABT | END | RET | ERR | AST | PRT 
                => buf.push_str(meta.intro),
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
        if let Some(data) = self.params_to_bytes(node) {
            if let Some(literal) = self.decode_bytes_literal(data) {
                return literal.text;
            }
            return format!("0x{}", hex::encode(data));
        }
        format!("0x{}", hex::encode(&node.para))
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
        // treat empty IR nodes as invisible placeholders
        if node.as_any().downcast_ref::<IRNodeEmpty>().is_some() {
            return String::new();
        }
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
        if let Some(pss) = node.as_any().downcast_ref::<IRNodeParam1Single>() {
            return self.print_param1_single(pss);
        }
        if let Some(line) = self.format_memory_put(node) {
            return line;
        }
        if let Some(line) = self.format_array_block(node) {
            return line;
        }
        let code: Bytecode = std_mem_transmute!(node.bytecode());
        if let Some(line) = self.format_call_instruction(node, code) {
            return line;
        }
        if let Some(line) = self.format_opty_double(code, node) {
            return line;
        }
        if let Some(line) = self.format_unary_triple(node) {
            return line;
        }
        self.print_descriptive(node)
    }
    /// Inline printing (equivalent to `print_sub_inline`).
    pub fn print_inline(&self, node: &dyn IRNode) -> String {
        if let Some(literal) = self.literal_from_node(node) {
            return literal.text;
        }
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
