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

#[allow(unused)]
#[derive(Clone)]
struct RecoveredLiteral {
    text: String,
    kind: LiteralKind,
    ty: Option<ValueTy>,
}

impl RecoveredLiteral {
    fn numeric<S: Into<String>>(value: S, ty: ValueTy) -> Self {
        Self {
            text: value.into(),
            kind: LiteralKind::Numeric,
            ty: Some(ty),
        }
    }

    fn text<S: Into<String>>(value: S) -> Self {
        Self {
            text: value.into(),
            kind: LiteralKind::Text,
            ty: Some(ValueTy::Bytes),
        }
    }

    fn address<S: Into<String>>(value: S) -> Self {
        Self {
            text: value.into(),
            kind: LiteralKind::Address,
            ty: Some(ValueTy::Address),
        }
    }
}

impl<'a> Formater<'a> {
    fn ensure_braced_block(&self, s: &str) -> String {
        let t = s.trim();
        maybe!(t.starts_with('{'), t.to_owned(), format!("{{ {} }}", t))
    }

    fn ensure_else_body(&self, s: &str) -> String {
        let t = s.trim();
        maybe!(
            t.starts_with('{') || t.starts_with("if "),
            t.to_owned(),
            format!("{{ {} }}", t)
        )
    }
    pub fn new(opt: &PrintOption<'a>) -> Self {
        Self { opt: opt.clone() }
    }

    fn child(&self) -> Self {
        Self {
            opt: self.opt.child(),
        }
    }

    fn with_tab(&self, t: usize) -> Self {
        Self {
            opt: self.opt.with_tab(t),
        }
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
            CALLTHIS | CALLSELF | CALLSUPER => self.resolve_func_name(&pss.para).and_then(|func| {
                let sig = calc_func_sign(&func);
                if pss.para.as_slice() != &sig[..] {
                    return None;
                }
                Some(match code {
                    CALLTHIS => format!("this.{}({})", func, args),
                    CALLSELF => format!("self.{}({})", func, args),
                    CALLSUPER => format!("super.{}({})", func, args),
                    _ => return None,
                })
            }),
            CALL | CALLVIEW | CALLPURE => {
                let (lib, func) = self.resolve_lib_func(pss.para[0], &pss.para[1..])?;
                let sig = calc_func_sign(&func);
                if pss.para.len() != 1 + sig.len() {
                    return None;
                }
                if &pss.para[1..] != &sig[..] {
                    return None;
                }
                Some(match code {
                    CALL => format!("{}.{}({})", lib, func, args),
                    CALLVIEW => format!("{}:{}({})", lib, func, args),
                    CALLPURE => format!("{}::{}({})", lib, func, args),
                    _ => return None,
                })
            }
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
                    return None;
                }
                _ => return None,
            }
        }
        None
    }

    fn extract_map_elements(
        &self,
        inst: Bytecode,
        subs: &[Box<dyn IRNode>],
    ) -> Option<Vec<(String, String)>> {
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
            if leaf.inst != PACKMAP {
                return None;
            }
        } else {
            return None;
        }
        let count_idx = num - 2;
        let count = num - 2;
        let expected = self.extract_const_usize(&*subs[count_idx])?;
        if count % 2 != 0 {
            return None;
        }
        // Historical IR variants exist:
        // - newer encoding stores total item count (k + v count),
        // - older encoding stores pair count.
        // Accept both to keep decompilation stable across artifacts.
        if expected != count && expected * 2 != count {
            return None;
        }
        let mut pairs = Vec::with_capacity(count / 2);
        for i in (0..count).step_by(2) {
            let k = self.print_inline(&*subs[i]);
            let v = self.print_inline(&*subs[i + 1]);
            pairs.push((k, v));
        }
        Some(pairs)
    }

    fn extract_list_elements(
        &self,
        inst: Bytecode,
        subs: &[Box<dyn IRNode>],
    ) -> Option<Vec<String>> {
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
        let expected = self.extract_const_usize(&*subs[count_idx])?;
        if expected != count {
            return None;
        }
        let mut elems = Vec::with_capacity(count);
        for node in &subs[..count] {
            elems.push(self.print_inline(&**node));
        }
        Some(elems)
    }

    fn format_call_args(&self, args: &[String]) -> String {
        // Join on argument boundaries (not line boundaries).
        // Some expressions (e.g. `{ ... }` blocks) legitimately contain newlines; splitting
        // by lines would corrupt the argument list and break roundtrip semantics.
        args.iter()
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()
            .join(", ")
    }

    fn collect_native_call_args(&self, node: &dyn IRNode, system_call: bool) -> Vec<String> {
        use Bytecode::*;
        let mut args = Vec::new();
        let mut current: &dyn IRNode = node;
        let helper = DecompilationHelper::new(&self.opt);
        loop {
            // `IRLIST` is used both for "packed argv lists" and for actual list/map literals.
            // For system/native/ext calls we use concat-argv mode; the argument node can
            // legitimately be a list literal, so flattening IRLIST here would corrupt
            // argument boundaries and even change call arity.
            if self.opt.flatten_call_list && !system_call {
                if let Some(list) = current.as_any().downcast_ref::<IRNodeArray>() {
                    if let Some(elements) = self.extract_list_elements(list.inst, &list.subs) {
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

    fn trim_default_call_args(&self, args: &mut Vec<String>, node: &dyn IRNode, system_call: bool) {
        use Bytecode::*;
        if !self.opt.hide_default_call_argv {
            return;
        }
        match node.bytecode() {
            x if x == PNIL as u8 => args.clear(),
            x if system_call && x == PNBUF as u8 => args.clear(),
            _ => {}
        }
    }

    fn build_call_args(&self, node: &dyn IRNode, system_call: bool) -> String {
        let mut args_list = self.collect_native_call_args(node, system_call);
        self.trim_default_call_args(&mut args_list, node, system_call);
        self.format_call_args(&args_list)
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
            if let Some(elements) = self.extract_list_elements(arr.inst, &arr.subs) {
                return Some(maybe!(
                    self.opt.flatten_array_list,
                    format!("{}[{}]", prefix, elements.join(", ")),
                    {
                        let args = elements.join(", ");
                        let mut buf = format!("{}list {{", prefix);
                        if !args.is_empty() {
                            buf.push(' ');
                            buf.push_str(&args);
                            buf.push(' ');
                        }
                        buf.push('}');
                        buf
                    }
                ));
            }
            if let Some(pairs) = self.extract_map_elements(arr.inst, &arr.subs) {
                let mut buf = format!("{}map {{", prefix);
                if !pairs.is_empty() {
                    buf.push('\n');
                    let child = self.child();
                    for (k, v) in pairs {
                        buf.push_str(&format!("{}{}: {},\n", child.line_prefix(), k, v));
                    }
                    buf.push_str(&self.opt.indent.repeat(self.opt.tab));
                }
                buf.push('}');
                return Some(buf);
            }
            if let Some(last) = arr.subs.last() {
                let code: Bytecode = std_mem_transmute!(last.bytecode());
                if matches!(code, LOG1 | LOG2 | LOG3 | LOG4) {
                    let args: Vec<String> = arr.subs[0..arr.subs.len() - 1]
                        .iter()
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
                } else {
                    // If we are trimming the file-level root block but not trimming param-unpack,
                    // the decompiled code may still use SourceMap parameter names (e.g. `amt`),
                    // which would be unbound without a `param { ... }` statement.
                    // Emit a lightweight slot-binding prelude: `var <name> $<i>` (no assignment).
                    let is_file_level_irblock = self.opt.tab == 0 && arr.inst == IRBLOCK;
                    if is_file_level_irblock && self.opt.map.is_some() {
                        // alloc could be at index 0 or 1
                        let mut alloc_index: Option<usize> = None;
                        for (i, s) in arr.subs.iter().enumerate().take(2) {
                            if s.bytecode() == ALLOC as u8 {
                                alloc_index = Some(i);
                                break;
                            }
                        }
                        let param_idx = alloc_index.map(|ai| ai + 1).unwrap_or(0);
                        if let Some(names) = helper.infer_param_names(arr, param_idx) {
                            for (i, name) in names.iter().enumerate() {
                                self.opt.mark_slot_put(i as u8);
                                buf.push_str(&format!(
                                    "{}var {} ${}",
                                    self.opt.indent.repeat(self.opt.tab + 1),
                                    name,
                                    i
                                ));
                                buf.push('\n');
                            }
                        }
                    }
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

        // Even when `trim_root_block` / `trim_param_unpack` are disabled, we still must keep
        // decompile->recompile closed.
        // IMPORTANT: when `trim_param_unpack == false`, we must NEVER emit `param { ... }`.
        // Instead, we keep the raw `UPLIST(PICK0,P0)` instruction in output, and (when SourceMap
        // provides parameter names) we emit lightweight slot-binding lines: `var <name> $<i>`.
        let is_file_level_irblock = self.opt.tab == 0 && arr.inst == IRBLOCK;
        // If `trim_param_unpack=true`, we may rewrite the canonical UPLIST node into `param { ... }`.
        // If `trim_param_unpack=false`, we must never emit `param { ... }`.
        let (param_rewrite_idx, param_rewrite_line) =
            if is_file_level_irblock && self.opt.trim_param_unpack {
                // alloc could be at index 0 or 1 (depending on placeholder insertion patterns)
                let mut alloc_index: Option<usize> = None;
                for (i, s) in arr.subs.iter().enumerate().take(2) {
                    if s.bytecode() == ALLOC as u8 {
                        alloc_index = Some(i);
                        break;
                    }
                }
                let idx = alloc_index.map(|ai| ai + 1).unwrap_or(0);
                // Mark param slots so later PUTs don't print as `var ...`.
                if let Some(names) = helper.infer_param_names(arr, idx) {
                    for i in 0..names.len() as u8 {
                        self.opt.mark_slot_put(i);
                    }
                }
                (Some(idx), helper.try_build_param_line(arr, idx))
            } else {
                (None, None)
            };

        let file_level_param_names = if is_file_level_irblock && !self.opt.trim_param_unpack {
            self.opt
                .map
                .and_then(|m| m.param_names().cloned())
                .filter(|n| !n.is_empty())
        } else {
            None
        };

        buf.push('{');
        if !arr.subs.is_empty() {
            buf.push('\n');

            if let Some(names) = file_level_param_names {
                // Bind param names to their canonical slots without changing runtime semantics.
                // Also pre-mark these slots so a later `PUT $i ...` does not print as a `var` declaration.
                for (i, name) in names.iter().enumerate() {
                    self.opt.mark_slot_put(i as u8);
                    buf.push_str(&format!(
                        "{}var {} ${}",
                        self.opt.indent.repeat(self.opt.tab + 1),
                        name,
                        i
                    ));
                    buf.push('\n');
                }
            }

            for (i, a) in arr.subs.iter().enumerate() {
                if let (Some(pidx), Some(line)) = (param_rewrite_idx, &param_rewrite_line) {
                    if i == pidx {
                        buf.push_str(line);
                        buf.push('\n');
                        continue;
                    }
                }
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
        if !matches!(
            code,
            CALL | CALLVIEW | CALLTHIS | CALLSELF | CALLSUPER | CALLPURE
        ) {
            return None;
        }
        let pre = self.line_prefix();
        let args = self.build_call_args(&*pss.subx, false);
        let meta = pss.inst.metadata();

        let default_body = match code {
            // IMPORTANT: keep `0x` prefix so the tokenizer parses it as bytes (0x....),
            // not as a decimal integer. Otherwise the recompiled 4-byte signature differs.
            CALL => format!(
                "call {}::0x{}({})",
                pss.para[0],
                ::hex::encode(&pss.para[1..]),
                args
            ),
            CALLTHIS => format!("callthis 0::0x{}({})", ::hex::encode(&pss.para), args),
            CALLSELF => format!("callself 0::0x{}({})", ::hex::encode(&pss.para), args),
            CALLSUPER => format!("callsuper 0::0x{}({})", ::hex::encode(&pss.para), args),
            CALLVIEW => format!(
                "callview {}::0x{}({})",
                pss.para[0],
                ::hex::encode(&pss.para[1..]),
                args
            ),
            CALLPURE => format!(
                "callpure {}::0x{}({})",
                pss.para[0],
                ::hex::encode(&pss.para[1..]),
                args
            ),
            _ => format!("{}({})", meta.intro, args),
        };

        let short_body = maybe!(
            self.opt.call_short_syntax,
            self.short_call_target(code, pss, &args),
            None
        );

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
        let sg = match OpTy::from_bytecode(d.inst) {
            Ok(t) => t.symbol(),
            Err(_) => return None,
        };
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
                CU8 | CU16 | CU32 | CU64 | CU128 | CBUF | RET | ERR | AST | PRT => {
                    let literal = self.literal_from_node(&*s.subx);
                    let substr = match &literal {
                        Some(lit) => lit.text.clone(),
                        None => self.print_inline(&*s.subx),
                    };
                    let operand = maybe!(
                        s.subx.level() > 0,
                        {
                            let t = substr.trim();
                            maybe!(
                                t.starts_with('(') && t.ends_with(')'),
                                substr.clone(),
                                format!("({})", substr)
                            )
                        },
                        substr.clone()
                    );
                    return Some(match s.inst {
                        CU8 => {
                            if self.opt.simplify_numeric_as_suffix && literal.is_some() {
                                format!("{}{}{}", pre, operand, "u8")
                            } else {
                                format!("{}{} as u8", pre, operand)
                            }
                        }
                        CU16 => {
                            if self.opt.simplify_numeric_as_suffix && literal.is_some() {
                                format!("{}{}{}", pre, operand, "u16")
                            } else {
                                format!("{}{} as u16", pre, operand)
                            }
                        }
                        CU32 => {
                            if self.opt.simplify_numeric_as_suffix && literal.is_some() {
                                format!("{}{}{}", pre, operand, "u32")
                            } else {
                                format!("{}{} as u32", pre, operand)
                            }
                        }
                        CU64 => {
                            if self.opt.simplify_numeric_as_suffix && literal.is_some() {
                                format!("{}{}{}", pre, operand, "u64")
                            } else {
                                format!("{}{} as u64", pre, operand)
                            }
                        }
                        CU128 => {
                            if self.opt.simplify_numeric_as_suffix && literal.is_some() {
                                format!("{}{}{}", pre, operand, "u128")
                            } else {
                                format!("{}{} as u128", pre, operand)
                            }
                        }
                        CBUF => format!("{}{} as bytes", pre, operand),
                        RET | ERR | AST | PRT => {
                            let meta = s.inst.metadata();
                            format!("{}{} {}", pre, meta.intro, substr)
                        }
                        _ => unreachable!(),
                    });
                }
                NOT => {
                    let mut substr = self.print_inline(&*s.subx);
                    if let Some((target, ty)) = self.format_is_components(&*s.subx) {
                        return Some(format!("{}{} is not {}", pre, target, ty));
                    }

                    // `!` has the highest precedence in fitsh (see `OpTy::NOT`),
                    // so when the operand is a lower-precedence expression we must
                    // parenthesize it to preserve semantics.
                    let need_wrap = {
                        let lv = s.subx.level();
                        lv > 0 && lv < OpTy::NOT.level()
                    };
                    if need_wrap {
                        let t = substr.trim();
                        if !(t.starts_with('(') && t.ends_with(')')) {
                            substr = format!("({})", substr);
                        }
                    }

                    return Some(format!("{}! {}", pre, substr));
                }
                _ => {
                    let meta = s.inst.metadata();
                    let argv = maybe!(meta.input == 0, s!(""), self.print_inline(&*s.subx));
                    return Some(format!("{}{}({})", pre, meta.intro, argv));
                }
            }
        }
        if let Some(d) = node.as_any().downcast_ref::<IRNodeDouble>() {
            if d.inst == ITEMGET {
                // `ITEMGET` is an expression; receiver must be printed inline.
                // Using `print_sub()` can inject newlines/indentation and break parsing.
                // Also, receiver precedence must be preserved: `(a + b)[0]` is not `a + b[0]`.
                let mut subxstr = self.print_inline(&*d.subx);
                if d.subx.level() > 0 {
                    let t = subxstr.trim();
                    if !(t.starts_with('(') && t.ends_with(')')) {
                        subxstr = format!("({})", t);
                    }
                }
                let subystr = self.print_inline(&*d.suby);
                return Some(format!("{}{}[{}]", self.line_prefix(), subxstr, subystr));
            }
            if d.inst == IRWHILE {
                let subxstr = self.print_inline(&*d.subx);
                let subystr = self.print_inner(&*d.suby);
                let body = self.ensure_braced_block(&subystr);
                return Some(format!("{}while {} {}", self.line_prefix(), subxstr, body));
            }
        }
        if let Some(t) = node.as_any().downcast_ref::<IRNodeTriple>() {
            if t.inst == Bytecode::CHOOSE {
                // IR stores CHOOSE as (yes, no, cond) for codegen/runtime semantics.
                // Source syntax is choose(cond, yes, no); invert when decompiling.
                let cond = self.print_inline(&*t.subz);
                let yes = self.print_inline(&*t.subx);
                let no = self.print_inline(&*t.suby);
                return Some(format!(
                    "{}choose({}, {}, {})",
                    self.line_prefix(),
                    cond,
                    yes,
                    no
                ));
            }
            if t.inst == IRIF || t.inst == IRIFR {
                let subxstr = self.print_inline(&*t.subx);
                let subystr = self.print_inner(&*t.suby);
                let body = self.ensure_braced_block(&subystr);
                let mut buf = format!("{}if {} {}", self.line_prefix(), subxstr, body);
                if t.subz.bytecode() != Bytecode::NOP as u8 {
                    let subzstr = self.print_inner(&*t.subz);
                    let elsebody = self.ensure_else_body(&subzstr);
                    buf.push_str(&format!(" else {}", elsebody));
                }
                return Some(buf);
            }
        }
        None
    }

    fn _cast_value_ty(inst: Bytecode) -> Option<ValueTy> {
        use Bytecode::*;
        match inst {
            CU8 => Some(ValueTy::U8),
            CU16 => Some(ValueTy::U16),
            CU32 => Some(ValueTy::U32),
            CU64 => Some(ValueTy::U64),
            CU128 => Some(ValueTy::U128),
            _ => None,
        }
    }

    fn literal_from_node(&self, node: &dyn IRNode) -> Option<RecoveredLiteral> {
        if !self.opt.recover_literals {
            return None;
        }
        use Bytecode::*;
        if let Some(single) = node.as_any().downcast_ref::<IRNodeSingle>() {
            match single.inst {
                CU8 => return self.numeric_literal_from_cast(&*single.subx, ValueTy::U8),
                CU16 => return self.numeric_literal_from_cast(&*single.subx, ValueTy::U16),
                CU32 => return self.numeric_literal_from_cast(&*single.subx, ValueTy::U32),
                CU64 => return self.numeric_literal_from_cast(&*single.subx, ValueTy::U64),
                CU128 => return self.numeric_literal_from_cast(&*single.subx, ValueTy::U128),
                _ => {}
            }
        }
        if let Some(leaf) = node.as_any().downcast_ref::<IRNodeLeaf>() {
            match leaf.inst {
                P0 => return Some(RecoveredLiteral::numeric("0", ValueTy::U8)),
                P1 => return Some(RecoveredLiteral::numeric("1", ValueTy::U8)),
                P2 => return Some(RecoveredLiteral::numeric("2", ValueTy::U8)),
                P3 => return Some(RecoveredLiteral::numeric("3", ValueTy::U8)),
                PTRUE => return Some(RecoveredLiteral::numeric("true", ValueTy::Bool)),
                PFALSE => return Some(RecoveredLiteral::numeric("false", ValueTy::Bool)),
                _ => {}
            }
        }
        if let Some(param1) = node.as_any().downcast_ref::<IRNodeParam1>() {
            if param1.inst == PU8 {
                return Some(RecoveredLiteral::numeric(
                    param1.para.to_string(),
                    ValueTy::U8,
                ));
            }
        }
        if let Some(param2) = node.as_any().downcast_ref::<IRNodeParam2>() {
            if param2.inst == PU16 {
                return Some(RecoveredLiteral::numeric(
                    u16::from_be_bytes(param2.para).to_string(),
                    ValueTy::U16,
                ));
            }
        }
        if let Some(params) = node.as_any().downcast_ref::<IRNodeParams>() {
            if let Some(data) = self.params_to_bytes(params) {
                if let Some(literal) = self.decode_bytes_literal(data) {
                    return Some(literal);
                }
            }
        }
        if let Some(single) = node.as_any().downcast_ref::<IRNodeParam1Single>() {
            use Bytecode::*;
            if single.inst == CTO && single.para == ValueTy::Address as u8 {
                return self.literal_from_node(&*single.subx);
            }
        }
        None
    }

    fn numeric_literal_from(&self, node: &dyn IRNode, ty: ValueTy) -> Option<RecoveredLiteral> {
        use Bytecode::*;
        if let Some(leaf) = node.as_any().downcast_ref::<IRNodeLeaf>() {
            match leaf.inst {
                P0 => return Some(RecoveredLiteral::numeric("0", ValueTy::U8)),
                P1 => return Some(RecoveredLiteral::numeric("1", ValueTy::U8)),
                P2 => return Some(RecoveredLiteral::numeric("2", ValueTy::U8)),
                P3 => return Some(RecoveredLiteral::numeric("3", ValueTy::U8)),
                PTRUE => return Some(RecoveredLiteral::numeric("true", ValueTy::Bool)),
                PFALSE => return Some(RecoveredLiteral::numeric("false", ValueTy::Bool)),
                _ => {}
            }
        }
        if let Some(param1) = node.as_any().downcast_ref::<IRNodeParam1>() {
            if param1.inst == PU8 {
                return Some(RecoveredLiteral::numeric(
                    param1.para.to_string(),
                    ValueTy::U8,
                ));
            }
        }
        if let Some(param2) = node.as_any().downcast_ref::<IRNodeParam2>() {
            if param2.inst == PU16 {
                return Some(RecoveredLiteral::numeric(
                    u16::from_be_bytes(param2.para).to_string(),
                    ValueTy::U16,
                ));
            }
        }
        if let Some(params) = node.as_any().downcast_ref::<IRNodeParams>() {
            if let Some(bytes) = self.params_to_bytes(params) {
                if let Some(value) = self.bytes_to_u128(bytes) {
                    return Some(RecoveredLiteral::numeric(value.to_string(), ty));
                }
            }
        }
        None
    }

    fn numeric_literal_from_cast(
        &self,
        node: &dyn IRNode,
        target: ValueTy,
    ) -> Option<RecoveredLiteral> {
        self.numeric_literal_from(node, target)
            .and_then(|lit| maybe!(lit.ty == Some(target), Some(lit), None))
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
                return Some(RecoveredLiteral::address(addr.to_readable()));
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
                let is_first = self.opt.mark_slot_put(node.para);
                let target = maybe!(
                    is_first,
                    {
                        let prefix = maybe!(
                            self.opt
                                .map
                                .map(|m| m.slot_is_var(node.para))
                                .unwrap_or(false),
                            "var",
                            maybe!(
                                self.opt
                                    .map
                                    .map(|m| m.slot_is_let(node.para))
                                    .unwrap_or(false),
                                "let",
                                "var"
                            )
                        );
                        let name = self.slot_name_display(node.para);
                        let slot_id_str = format!("${}", node.para);
                        format!("{} {} {}", prefix, name, slot_id_str)
                    },
                    self.slot_name_display(node.para)
                );
                format!("{} = {}", target, substr)
            }
            XOP => self.format_local_param(node, local_operand_param_parse),
            XLG => self.format_local_param(node, local_logic_param_parse),
            EXTENV => self.format_extend_call(node, &CALL_EXTEND_ENV_DEFS),
            EXTVIEW => self.format_extend_call(node, &CALL_EXTEND_VIEW_DEFS),
            EXTACTION => self.format_extend_call(node, &CALL_EXTEND_ACTION_DEFS),
            NTFUNC => {
                let ntfn: NativeFunc = std_mem_transmute!(node.para);
                let argv = self.build_call_args(&*node.subx, true);
                format!("{}({})", ntfn.name(), argv)
            }
            NTENV => {
                let ntfn: NativeEnv = std_mem_transmute!(node.para);
                let args = maybe!(
                    self.opt.hide_default_call_argv,
                    String::new(),
                    "\"\"".to_string()
                );
                format!("{}({})", ntfn.name(), args)
            }
            _ => {
                let substr = self.print_inline(&*node.subx);
                format!("{}({}, {})", meta.intro, node.para, substr)
            }
        };
        format!("{}{}", pre, body)
    }

    fn print_param2_single(&self, node: &IRNodeParam2Single) -> String {
        let pre = self.opt.indent.repeat(self.opt.tab);
        let meta = node.inst.metadata();
        let substr = self.print_sub(&*node.subx);
        format!(
            "{}{}({}, {}, {})",
            pre, meta.intro, node.para[0], node.para[1], substr
        )
    }

    fn format_extend_call(
        &self,
        node: &IRNodeParam1Single,
        defs: &[(u8, &'static str, ValueTy, usize)],
    ) -> String {
        let id = node.para;
        let Some(f) = search_ext_by_id(id, defs) else {
            return format!(
                "/* unknown external call id: {} */ __unknown_ext_{}_()",
                id, id
            );
        };
        let argv = self.build_call_args(&*node.subx, true);
        format!("{}({})", f.1, argv)
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
            PTRUE => buf.push_str("true"),
            PFALSE => buf.push_str("false"),
            PNIL => buf.push_str("nil"),
            PNBUF => buf.push_str("\"\""),
            NEWLIST => buf.push_str("[]"),
            IRBREAK => buf.push_str("break"),
            IRCONTINUE => buf.push_str("continue"),
            ABT | END | RET | ERR | AST | PRT => buf.push_str(meta.intro),
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
            GET => buf.push_str(&self.slot_name_display(node.para)),
            EXTENV => {
                let ary = CALL_EXTEND_ENV_DEFS;
                let f = search_ext_name_by_id(node.para, &ary);
                buf.push_str(&format!("{}()", f));
            }
            NTENV => {
                let ntfn: NativeEnv = std_mem_transmute!(node.para);
                if self.opt.hide_default_call_argv {
                    buf.push_str(&format!("{}()", ntfn.name()));
                } else {
                    buf.push_str(&format!("{}(\"\")", ntfn.name()));
                }
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
                buf.push_str(&format!(
                    "{}({}, {})",
                    meta.intro, node.para[0], node.para[1]
                ));
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
                // IMPORTANT: keep `0x` prefix so tokenizer parses it as bytes (0x....),
                // not as a decimal integer/identifier.
                buf.push_str(&format!("callcode {}::0x{}", i, f));
                // Source syntax requires a trailing `end` token for callcode statements.
                buf.push_str(&format!("\n{}end", self.line_prefix()));
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
        // codegen-only placeholder must never appear in decompilation
        if node
            .as_any()
            .downcast_ref::<IRNodeTopStackValue>()
            .is_some()
        {
            panic!("IRNodeTopStackValue is codegen-only and must not be decompiled/printed");
        }
        if let Some(leaf) = node.as_any().downcast_ref::<IRNodeLeaf>() {
            return self.format_leaf(leaf);
        }
        if let Some(bytecodes) = node.as_any().downcast_ref::<IRNodeBytecodes>() {
            let buf = self.opt.indent.repeat(self.opt.tab);
            let codes = match bytecodes.codes.bytecode_print(false) {
                Ok(s) => s.trim_end().to_owned(),
                Err(_) => format!("0x{}", hex::encode(&bytecodes.codes)),
            };
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
            return format!("{}{}({}, {}, {})", prefix, meta.intro, subx, suby, subz);
        }
        format!("{}{}", prefix, meta.intro)
    }

    /// Main entry for descriptive printing.
    pub fn print(&self, node: &dyn IRNode) -> String {
        if node
            .as_any()
            .downcast_ref::<IRNodeTopStackValue>()
            .is_some()
        {
            panic!("IRNodeTopStackValue is codegen-only and must not be decompiled/printed");
        }

        // Emit file-level `lib ...` declarations once, at the top-level.
        // Do NOT rely on `tab == 0 && IRBLOCK` injection during block formatting, because
        // inline contexts may also print with `tab == 0`.
        let is_top_level = self.opt.tab == 0;
        let is_file_level_irblock = is_top_level
            && node
                .as_any()
                .downcast_ref::<IRNodeArray>()
                .is_some_and(|arr| arr.inst == Bytecode::IRBLOCK);

        let prelude = if self.opt.emit_lib_prelude && is_file_level_irblock {
            if let Some(map) = self.opt.map {
                let mut prefix = String::new();
                for (idx, info) in map.lib_entries() {
                    let line = match &info.address {
                        Some(addr) => {
                            format!("lib {} = {}: {}\n", info.name, idx, addr.to_readable())
                        }
                        None => format!("lib {} = {}\n", info.name, idx),
                    };
                    prefix.push_str(&line);
                }
                prefix
            } else {
                String::new()
            }
        } else {
            String::new()
        };

        let res = self.print_inner(node);

        // Emit source-map-derived `const ...` definitions only at file-level.
        // Nested `print()` calls are used to render block items with indentation;
        // draining and emitting `const` there would change scoping/ordering and may
        // break roundtrip semantics.
        if is_top_level {
            let pending = self.opt.take_pending_consts();
            if !pending.is_empty() {
                let mut defs = String::new();
                let prefix = self.line_prefix();
                if let Some(map) = self.opt.map {
                    for name in pending {
                        if self.opt.mark_const_printed(name.clone()) {
                            if let Some(value) = map.get_const_value(&name) {
                                defs.push_str(&format!("{}const {} = {}\n", prefix, name, value));
                            }
                        }
                    }
                }
                return format!("{}{}{}", prelude, defs, res);
            }
        }

        format!("{}{}", prelude, res)
    }

    fn print_inner(&self, node: &dyn IRNode) -> String {
        if node
            .as_any()
            .downcast_ref::<IRNodeTopStackValue>()
            .is_some()
        {
            panic!("IRNodeTopStackValue is codegen-only and must not be decompiled/printed");
        }
        if let Some(wrap) = node.as_any().downcast_ref::<IRNodeWrapOne>() {
            return format!("({})", self.print_inline(&*wrap.node));
        }
        if let Some(pss) = node.as_any().downcast_ref::<IRNodeParam1Single>() {
            return self.print_param1_single(pss);
        }
        if let Some(pss) = node.as_any().downcast_ref::<IRNodeParam2Single>() {
            return self.print_param2_single(pss);
        }
        if let Some(line) = self.format_memory_put(node) {
            return line;
        }
        if let Some(line) = self.format_array_block(node) {
            return line;
        }
        let code: Bytecode = std_mem_transmute!(node.bytecode());
        if code == Bytecode::NEWLIST {
            return maybe!(
                self.opt.flatten_array_list,
                format!("{}[]", self.line_prefix()),
                format!("{}list {{}}", self.line_prefix())
            );
        }
        if code == Bytecode::NEWMAP {
            return format!("{}map {{}}", self.line_prefix());
        }
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
        if node
            .as_any()
            .downcast_ref::<IRNodeTopStackValue>()
            .is_some()
        {
            panic!("IRNodeTopStackValue is codegen-only and must not be decompiled/printed");
        }

        // Cast nodes need explicit render to keep `100u64`/`100 as u64` shape.
        // Returning only recovered literal text here would drop the type suffix/cast.
        if let Some(single) = node.as_any().downcast_ref::<IRNodeSingle>() {
            use Bytecode::*;
            match single.inst {
                CU8 | CU16 | CU32 | CU64 | CU128 => {
                    if let Some(literal) = self.literal_from_node(node) {
                        // `push_num` encodes larger integers as CU32/CU64/CU128 over raw
                        // PBUF bytes. That cast is an encoding detail, not source syntax.
                        // Keep decompilation stable by printing the plain number in this case.
                        let encoded_numeric = single
                            .subx
                            .as_any()
                            .downcast_ref::<IRNodeParams>()
                            .is_some_and(|p| matches!(p.inst, Bytecode::PBUF | Bytecode::PBUFL));
                        if encoded_numeric {
                            return literal.text;
                        }

                        let suffix = match single.inst {
                            CU8 => "u8",
                            CU16 => "u16",
                            CU32 => "u32",
                            CU64 => "u64",
                            CU128 => "u128",
                            _ => unreachable!(),
                        };
                        return maybe!(
                            self.opt.simplify_numeric_as_suffix,
                            format!("{}{}", literal.text, suffix),
                            format!("{} as {}", literal.text, suffix)
                        );
                    }
                }
                _ => {}
            }
        }

        if let Some(literal) = self.literal_from_node(node) {
            if let Some(map) = self.opt.map {
                if let Some(name) = map.get_const_name(&literal.text) {
                    if !self.opt.is_const_printed(name) {
                        self.opt.add_pending_const(name.clone());
                    }
                    return name.clone();
                }
            }
            return literal.text;
        }
        // Inline contexts must not apply root-block trimming or emit lib prelude.
        // Otherwise, an `IRBLOCK` printed with `tab == 0` may lose its `{}` wrapper,
        // which can break parsing/semantics when used as an expression (e.g. call args).
        let mut opt = self.opt.with_tab(0);
        opt.trim_root_block = false;
        opt.emit_lib_prelude = false;
        let inline = Self { opt };
        // IMPORTANT: inline printing must never emit file-level prelude or const
        // definitions. Those are handled by the outer/top-level `print()` only.
        // Using `print()` here would drain `pending consts` and inject `const ...`
        // lines into expression contexts (e.g. call args), breaking parsing/semantics.
        let substr = inline.print_inner(node);
        if substr.contains('\n') {
            let t = substr.trim();
            // Preserve block braces for expression blocks.
            // Stripping `{ ... }` breaks roundtrip for multi-statement blocks
            // used in inline contexts (e.g. `var x = { print 1; 0 }`).
            if t.starts_with('{') && t.ends_with('}') && t.len() >= 2 {
                return t.to_owned();
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
        } else {
            emp
        }
    }

    pub fn print_subx_suby_op(&self, dbl: &IRNodeDouble, op: &str) -> String {
        let inline_opt = self.with_tab(0);
        let mut subx = inline_opt.print_inline(&*dbl.subx);
        let mut suby = inline_opt.print_inline(&*dbl.suby);
        let wrapx = dbl.subx.as_any().downcast_ref::<IRNodeWrapOne>().is_some();
        let wrapy = dbl.suby.as_any().downcast_ref::<IRNodeWrapOne>().is_some();
        let (clv, is_right_assoc) = match OpTy::from_bytecode(dbl.inst) {
            Ok(t) => (t.level(), t.is_right_assoc()),
            _ => (0, false),
        };
        let llv = dbl.subx.level();
        let rlv = dbl.suby.level();

        // Parenthesize children to preserve the original IR tree semantics.
        // - For left-associative ops, wrap the right child on equal precedence.
        // - For right-associative ops (currently only `**`), wrap the left child on equal precedence.
        let need_wrap_left =
            clv > 0 && llv > 0 && !wrapx && (clv > llv || (is_right_assoc && clv == llv));
        if need_wrap_left {
            subx = format!("({})", &subx);
        }

        let need_wrap_right =
            clv > 0 && rlv > 0 && !wrapy && (clv > rlv || (!is_right_assoc && clv == rlv));
        if need_wrap_right {
            suby = format!("({})", &suby);
        }
        format!("{} {} {}", subx, op, suby)
    }

    // Added: local `ascii_show_string` implementation to avoid relying on external imports.
    fn ascii_show_string(&self, data: &[u8]) -> Option<String> {
        if data.is_empty() {
            return Some(String::new());
        }
        // Determine printable ASCII (common newline/tab are allowed).
        if data
            .iter()
            .all(|&b| (b >= 0x20 && b <= 0x7E) || b == 0x0a || b == 0x0d || b == 0x09)
        {
            match std::str::from_utf8(data) {
                Ok(s) => Some(s.to_string()),
                Err(_) => None,
            }
        } else {
            None
        }
    }
}
