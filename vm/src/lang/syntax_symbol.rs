#[allow(dead_code)]
impl Syntax {
    pub fn bind_local_assign(
        &mut self,
        s: String,
        v: Box<dyn IRNode>,
        kind: SlotKind,
    ) -> Ret<Box<dyn IRNode>> {
        let idx = self.local_alloc;
        self.bind_local_assign_replace(s, idx, v, kind)
    }

    pub fn bind_local_assign_replace(
        &mut self,
        s: String,
        idx: u8,
        v: Box<dyn IRNode>,
        kind: SlotKind,
    ) -> Ret<Box<dyn IRNode>> {
        use Bytecode::*;
        self.bind_local(s, idx, kind)?;
        Ok(push_single_p1(PUT, idx, v))
    }

    fn parse_local_statement(&mut self, kind: SlotKind, err_msg: &str) -> Ret<Box<dyn IRNode>> {
        use super::rt::Token::*;
        let e = errf!("{}", err_msg);
        let gidx = |nxt: &Token| {
            let mut lcalc: Option<u8> = None;
            if let Identifier(num) = nxt.clone() {
                if start_with_char(&num, '$') {
                    if let Ok(idx) = num.trim_start_matches('$').parse::<u8>() {
                        lcalc = Some(idx);
                    };
                }
            }
            lcalc
        };
        let Identifier(id) = self.next()? else {
            return e;
        };
        let vk = id.clone();
        let mut nxt = self.next()?;
        let mut idx = None;
        let mut val = None;
        if let Some(i) = gidx(&nxt) {
            idx = Some(i);
            nxt = self.next()?;
        }
        if let Keyword(KwTy::Assign) = nxt {
            let v = self.item_must(0)?;
            if !v.hasretval() {
                return errf!(
                    "{} initializer must be expressions with return values; do not use bind/var/let declarations directly",
                    match kind {
                        SlotKind::Var => "var",
                        SlotKind::Let => "let",
                        _ => "local",
                    }
                );
            }
            val = Some(v);
        } else {
            self.idx -= 1;
        }
        match (idx, val) {
            (Some(i), Some(v)) => self.bind_local_assign_replace(vk, i, v, kind),
            (.., Some(v)) => self.bind_local_assign(vk, v, kind),
            (Some(i), ..) => self.bind_local(vk, i, kind),
            _ => return e,
        }
    }

    // ret empty
    pub fn bind_local(&mut self, s: String, idx: u8, kind: SlotKind) -> Ret<Box<dyn IRNode>> {
        self.reserve_slot(idx)?;
        if idx >= self.local_alloc {
            if idx == u8::MAX {
                return errf!("slot {} exceeds limit", idx);
            }
            self.local_alloc = idx + 1;
        }
        let mutable = matches!(kind, SlotKind::Let) == false;
        self.register_slot_symbol(s.clone(), idx, mutable)?;
        self.emit.source_map.register_slot(idx, s)?;
        Ok(push_empty())
    }

    fn register_slot_symbol(&mut self, s: String, idx: u8, mutable: bool) -> Rerr {
        if let Some(..) = self.symbols.get(&s) {
            return errf!("symbol '{}' already bound", s);
        }
        self.symbols.insert(s, SymbolEntry::Slot(idx, mutable));
        Ok(())
    }

    fn register_bind_symbol(&mut self, s: String, entry: SymbolEntry) -> Rerr {
        if let Some(SymbolEntry::Slot(_, _)) = self.symbols.get(&s) {
            return errf!("cannot rebind slot '{}' with bind", s);
        }
        self.symbols.insert(s, entry);
        Ok(())
    }

    pub fn bind_macro(&mut self, s: String, v: Box<dyn IRNode>) -> Rerr {
        self.register_bind_symbol(s, SymbolEntry::Bind(v))?;
        Ok(())
    }

    fn reserve_slot(&mut self, idx: u8) -> Rerr {
        if self.slot_used.contains(&idx) {
            return errf!("slot {} already bound", idx);
        }
        self.slot_used.insert(idx);
        Ok(())
    }

    pub fn link_local(&self, s: &String) -> Ret<Box<dyn IRNode>> {
        let text = s.clone();
        if let Some(SymbolEntry::Slot(i, _)) = self.symbols.get(s) {
            return Ok(push_local_get(*i, text));
        }
        if let Some(idx) = Self::parse_slot_str(s) {
            return Ok(push_local_get(idx, text));
        }
        errf!("cannot find symbol '{}'", s)
    }

    fn slot_info(&self, s: &String) -> Ret<(u8, bool)> {
        if let Some(SymbolEntry::Slot(idx, mutable)) = self.symbols.get(s) {
            return Ok((*idx, *mutable));
        }
        if let Some(idx) = Self::parse_slot_str(s) {
            return Ok((idx, true));
        }
        errf!("cannot find symbol '{}'", s)
    }

    pub fn link_bind(&self, s: &String) -> Ret<Box<dyn IRNode>> {
        match self.symbols.get(s) {
            Some(SymbolEntry::Bind(expr)) => Ok(dyn_clone::clone_box(expr.as_ref())),
            _ => errf!("cannot find or relink symbol '{}'", s),
        }
    }

    pub fn link_symbol(&mut self, s: &String) -> Ret<Box<dyn IRNode>> {
        match self.symbols.get(s) {
            Some(SymbolEntry::Bind(_)) => self.link_bind(s),
            Some(SymbolEntry::Const(node)) => Ok(dyn_clone::clone_box(node.as_ref())),
            _ => self.link_local(s),
        }
    }

    pub fn save_local(&mut self, s: &String, v: Box<dyn IRNode>) -> Ret<Box<dyn IRNode>> {
        use Bytecode::*;
        let (i, mutable) = self.slot_info(s)?;
        if !mutable {
            return errf!("cannot assign to immutable symbol '{}'", s);
        }
        self.emit.source_map.mark_slot_mutated(i);
        Ok(push_single_p1(PUT, i, v))
    }

    pub fn assign_local(
        &mut self,
        s: &String,
        v: Box<dyn IRNode>,
        op: &Token,
    ) -> Ret<Box<dyn IRNode>> {
        use Bytecode::*;
        use KwTy::*;
        let (i, mutable) = self.slot_info(s)?;
        if !mutable {
            return errf!("cannot assign to immutable symbol '{}'", s);
        }
        self.emit.source_map.mark_slot_mutated(i);
        let (lxop, arith) = match op {
            Keyword(AsgAdd) => (LxOp::Add, ADD),
            Keyword(AsgSub) => (LxOp::Sub, SUB),
            Keyword(AsgMul) => (LxOp::Mul, MUL),
            Keyword(AsgDiv) => (LxOp::Div, DIV),
            _ => unreachable!(),
        };
        if i < 64 {
            let mark = encode_local_operand_mark(lxop, i).map_err(Error::from)?;
            return Ok(push_single_p1(XOP, mark, v));
        }
        // $0 = $0 + 1
        let getv = push_local_get(i, s!(""));
        let opsv = Box::new(IRNodeDouble {
            hrtv: true,
            inst: arith,
            subx: getv,
            suby: v,
        });
        Ok(push_single_p1(PUT, i, opsv))
    }
}
