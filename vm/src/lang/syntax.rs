use std::collections::{HashMap, HashSet};
use std::convert::TryInto;
use hex;

enum SymbolEntry {
    Slot(u8, bool),
    Bind(Box<dyn IRNode>),
    Const(Box<dyn IRNode>),
}

#[allow(dead_code)]
#[derive(Default)]
pub struct Syntax {
    tokens: Vec<Token>,
    idx: usize,
    symbols: HashMap<String, SymbolEntry>,
    slot_used: HashSet<u8>,
    bdlibs: HashMap<String, (u8, Option<field::Address>)>,
    local_alloc: u8,
    check_op: bool,
    expect_retval: bool,
    is_ircode: bool,  // true for ircode mode, false for bytecode mode
    // leftv: Box<dyn AST>,
    irnode: IRNodeArray, // replaced IRNodeArray -> IRNodeArray
    source_map: SourceMap,
    // External injected
    ext_params: Option<Vec<(String, ValueTy)>>,
    ext_libs: Option<Vec<(String, u8, Option<field::Address>)>>,
}


#[allow(dead_code)]
impl Syntax {
    fn opcode_irblock(keep_retval: bool) -> Bytecode {
        use Bytecode::*;
        maybe!(keep_retval, IRBLOCKR, IRBLOCK)
    }

    fn opcode_irif(keep_retval: bool) -> Bytecode {
        use Bytecode::*;
        maybe!(keep_retval, IRIFR, IRIF)
    }

    fn build_irlist(subs: Vec<Box<dyn IRNode>>) -> Ret<IRNodeArray> {
        use Bytecode::*;
        IRNodeArray::from_vec(subs, IRLIST)
    }

    fn parse_delimited_value_exprs(
        &mut self,
        open: char,
        close: char,
        err_msg: &'static str,
    ) -> Ret<Vec<Box<dyn IRNode>>> {
        let end = self.tokens.len() - 1; // trailing synthetic sentinel
        if self.idx >= end {
            return errf!("{}", err_msg);
        }
        let Partition(c) = &self.tokens[self.idx] else {
            return errf!("{}", err_msg);
        };
        if *c != open {
            return errf!("{}", err_msg);
        }
        self.idx += 1;

        let mut subs: Vec<Box<dyn IRNode>> = Vec::new();
        let mut block_err = false;
        let mut closed = false;
        self.with_expect_retval(true, |s| {
            loop {
                if s.idx >= end {
                    block_err = true;
                    break;
                }
                let nxt = &s.tokens[s.idx];
                if let Partition(sp) = nxt {
                    if *sp == close {
                        closed = true;
                        s.idx += 1;
                        break;
                    } else if matches!(sp, '}' | ')' | ']') {
                        block_err = true;
                        break;
                    }
                }
                let Some(li) = s.item_may()? else {
                    break;
                };
                subs.push(li);
            }
            Ok::<(), Error>(())
        })?;

        if block_err || !closed {
            return errf!("{}", err_msg);
        }
        for arg in &subs {
            arg.checkretval()?;
        }
        Ok(subs)
    }

    fn build_list_node(&mut self, mut subs: Vec<Box<dyn IRNode>>) -> Ret<Box<dyn IRNode>> {
        use Bytecode::*;
        let num = subs.len();
        if num == 0 {
            return Ok(push_inst(NEWLIST));
        }
        subs.push(push_num(num as u128));
        subs.push(push_inst(PACKLIST));
        let arys = Self::build_irlist(subs)?;
        Ok(Box::new(arys))
    }



    /*
    pub fn bind_uses(&mut self, s: String, adr: Vec<u8>) -> Rerr {
        if let Some(..) = self.bduses.get(&s) {
            return errf!("<use> cannot repeat bind the symbol '{}'", s)
        }
        let addr = Address::from_vec(adr);
        addr.must_contract()?;
        self.bduses.insert(s, addr);
        Ok(())
    }

    pub fn link_use(&self, s: &String) -> Ret<Vec<u8>> {
        match self.bduses.get(s) {
            Some(i) => Ok(i.to_vec()),
            _ =>  errf!("cannot find any use bind '{}'", s)
        }
    }
    */
    

    fn next(&mut self) -> Ret<Token> {
        if self.idx >= self.tokens.len() {
            return errf!("item_with_left get next token error")
        }
        let nxt = &self.tokens[self.idx];
        self.idx += 1;
        Ok(nxt.clone())
    }

    fn with_expect_retval<F, R>(&mut self, expect: bool, f: F) -> R
    where F: FnOnce(&mut Self) -> R
    {
        let prev = self.expect_retval;
        self.expect_retval = expect;
        let res = f(self);
        self.expect_retval = prev;
        res
    }

    pub fn link_lib(&self, s: &String) -> Ret<u8> {
        match self.bdlibs.get(s).map(|d|d.0) {
            Some(i) => Ok(i),
            _ =>  errf!("cannot find any lib bind '{}'", s)
        }
    }

    pub fn bind_lib(&mut self, s: String, idx: u8, adr: Option<field::Address>) -> Rerr {
        if let Some(..) = self.bdlibs.get(&s) {
            return errf!("<use> cannot repeat bind the symbol '{}'", s)
        }
        if let Some(adr) = adr {
            adr.must_contract()?;
        }
        self.bdlibs.insert(s.clone(), (idx, adr.clone()));
        self.source_map.register_lib(idx, s, adr)?;
        Ok(())
    }

    fn parse_lib_index_token(token: &Token) -> Ret<u8> {
        if let Integer(n) = token {
            if *n > u8::MAX as u128 {
                return errf!("call index overflow")
            }
            return Ok(*n as u8)
        }
        errf!("call index must be integer")
    }

    fn parse_fn_sig_str(s: &str) -> Ret<[u8;4]> {
        let hex = s.strip_prefix("0x").unwrap_or(s);
        if hex.len() != 8 {
            return errf!("function signature must be 8 hex digits, got '{}'", s)
        }
        let bytes = match hex::decode(hex) {
            Ok(b) => b,
            Err(_) => return errf!("function signature '{}' decode error", s),
        };
        let arr: [u8;4] = match bytes.as_slice().try_into() {
            Ok(a) => a,
            Err(_) => return errf!("function signature expects 4 bytes"),
        };
        Ok(arr)
    }

    fn parse_fn_sig_token(token: &Token) -> Ret<[u8;4]> {
        match token {
            Identifier(name) => Self::parse_fn_sig_str(name),
            Bytes(bytes) if bytes.len() == 4 => {
                let arr: [u8;4] = bytes.as_slice().try_into().unwrap();
                Ok(arr)
            }
            Integer(n) if *n <= u32::MAX as u128 => {
                let v = *n as u32;
                Ok(v.to_be_bytes())
            }
            Bytes(..) => errf!("function signature bytes must be exactly 4 bytes"),
            _ => errf!("function signature must be hex identifier, decimal <u32>, or 4-byte literal"),
        }
    }

    fn parse_slot_str(id: &str) -> Option<u8> {
        if start_with_char(id, '$') {
            if let Ok(idx) = id.trim_start_matches('$').parse::<u8>() {
                return Some(idx);
            }
        }
        None
    }

    fn literal_value_type(node: &dyn IRNode) -> Option<ValueTy> {
        use Bytecode::*;
        if let Some(leaf) = node.as_any().downcast_ref::<IRNodeLeaf>() {
            return match leaf.inst {
                P0 | P1 | P2 | P3 => Some(ValueTy::U8),
                PTRUE | PFALSE => Some(ValueTy::Bool),
                _ => None,
            };
        }
        if let Some(param1) = node.as_any().downcast_ref::<IRNodeParam1>() {
            if param1.inst == PU8 {
                return Some(ValueTy::U8);
            }
        }
        if let Some(param2) = node.as_any().downcast_ref::<IRNodeParam2>() {
            if param2.inst == PU16 {
                return Some(ValueTy::U16);
            }
        }
        if let Some(single) = node.as_any().downcast_ref::<IRNodeSingle>() {
            return match single.inst {
                CU8 => Some(ValueTy::U8),
                CU16 => Some(ValueTy::U16),
                CU32 => Some(ValueTy::U32),
                CU64 => Some(ValueTy::U64),
                CU128 => Some(ValueTy::U128),
                _ => None,
            };
        }
        None
    }

    pub fn bind_local_assign(&mut self, s: String, v: Box<dyn IRNode>, kind: SlotKind) -> Ret<Box<dyn IRNode>> {
        let idx = self.local_alloc;
        self.bind_local_assign_replace(s, idx, v, kind)
    }

    pub fn bind_local_assign_replace(&mut self, s: String, idx: u8, v: Box<dyn IRNode>, kind: SlotKind) -> Ret<Box<dyn IRNode>> {
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
            return e
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
            (.., Some(v))      => self.bind_local_assign(vk, v, kind),
            (Some(i), ..)      => self.bind_local(vk, i, kind),
            _ => return e
        }
    }

    // ret empty
    pub fn bind_local(&mut self, s: String, idx: u8, kind: SlotKind) -> Ret<Box<dyn IRNode>> {
        self.reserve_slot(idx)?;
        if idx >= self.local_alloc {
            if idx == u8::MAX {
                return errf!("slot {} exceeds limit", idx)
            }
            self.local_alloc = idx + 1;
        }
        let mutable = matches!(kind, SlotKind::Let) == false;
        self.register_slot_symbol(s.clone(), idx, mutable)?;
        self.source_map.register_slot(idx, s, kind)?;
        Ok(push_empty())
    }

    fn register_slot_symbol(&mut self, s: String, idx: u8, mutable: bool) -> Rerr {
        if let Some(..) = self.symbols.get(&s) {
            return errf!("symbol '{}' already bound", s)
        }
        self.symbols.insert(s, SymbolEntry::Slot(idx, mutable));
        Ok(())
    }

    fn register_bind_symbol(&mut self, s: String, entry: SymbolEntry) -> Rerr {
        if let Some(SymbolEntry::Slot(_, _)) = self.symbols.get(&s) {
            return errf!("cannot rebind slot '{}' with bind", s)
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
            return errf!("slot {} already bound", idx)
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
            return errf!("cannot assign to immutable symbol '{}'", s)
        }
        self.source_map.mark_slot_mutated(i);
        Ok(push_single_p1(PUT, i, v))
    }
    
    pub fn assign_local(&mut self, s: &String, v: Box<dyn IRNode>, op: &Token) -> Ret<Box<dyn IRNode>> {
        use Bytecode::*;
        use KwTy::*;
        let (i, mutable) = self.slot_info(s)?;
        if !mutable {
            return errf!("cannot assign to immutable symbol '{}'", s)
        }
        self.source_map.mark_slot_mutated(i);
        if i < 64 {
            let mark = i | match op {
                Keyword(AsgAdd) => 0b00000000,
                Keyword(AsgSub) => 0b01000000,
                Keyword(AsgMul) => 0b10000000,
                Keyword(AsgDiv) => 0b11000000,
                _ => unreachable!(),
            };
            return Ok(push_single_p1(XOP, mark, v))
        }
        // $0 = $0 + 1
        let getv = push_local_get(i, s!(""));
        let opsv = Box::new(IRNodeDouble{hrtv: true, inst: match op {
            Keyword(AsgAdd) => ADD,
            Keyword(AsgSub) => SUB,
            Keyword(AsgMul) => MUL,
            Keyword(AsgDiv) => DIV,
            _ => unreachable!()
        }, subx: getv, suby: v});
        Ok(push_single_p1(PUT, i, opsv))
    }
    

    pub fn new(mut tokens: Vec<Token>) -> Self {
        use Bytecode::*;
        tokens.push(Token::Partition('}'));
        Self {
            tokens,
            irnode: IRNodeArray::with_opcode(IRBLOCK), // was IRNodeArray::new()
            check_op: true,
            ..Default::default()
        }
    }


    pub fn item_with_left(&mut self, mut left: Box<dyn IRNode>) -> Ret<Box<dyn IRNode>> {
        use Bytecode::*;
        use KwTy::*;
        let max = self.tokens.len();
        let sfptr = self as *mut Syntax;
        if self.idx >= self.tokens.len() {
            return Ok(left) // end
        }
        macro_rules! next { () => {{
            if self.idx >= max {
                return errf!("item with left get next token error")
            }
            let nxt = self.tokens[self.idx].clone();
            self.idx += 1;
            nxt
        }}}
        loop {
            if self.idx >= max {
                break
            }
            let nxt = next!();
            match nxt {
                Partition('[') => {
                    // Allow indexing on any value expression, not just identifiers.
                    // This keeps decompile->recompile closed for `ITEMGET` nodes.
                    left.checkretval()?; // receiver must be a value expression
                    let k = self.item_must(0)?;
                    k.checkretval()?; // key must be a value expression
                    let Partition(']') = next!() else {
                        return errf!("item get statement format error")
                    };
                    left = Box::new(IRNodeDouble { hrtv: true, inst: ITEMGET, subx: left, suby: k });
                }
                Keyword(KwTy::Assign) 
                | Keyword(AsgAdd) 
                | Keyword(AsgSub)
                | Keyword(AsgMul)
                | Keyword(AsgDiv) => {
                    let e = errf!("assign statement format error");
                    let mut id: Option<String> = None; 
                    if let Some(ir) = left.as_any().downcast_ref::<IRNodeParam1>() {
                        id = Some(ir.as_text().clone());
                    };
                    if let Some(ir) = left.as_any().downcast_ref::<IRNodeLeaf>() {
                        id = Some(ir.as_text().clone());
                    };
                    let Some(id) = id else {
                        return e
                    };
                    let v = unsafe { (&mut *sfptr).item_must(0)? };
                    v.checkretval()?; // must retv
                    left = match nxt {
                        Keyword(KwTy::Assign) => self.save_local(&id, v)?,
                        _ => self.assign_local(&id, v, &nxt)?,
                    };
                }
                Keyword(As) => {
                    left.checkretval()?; // must retv
                    let e = errf!("<as> express format error");
                    let nk = next!();
                    let hrtv = true;
                    let target_ty = match nk {
                        Keyword(U8) => Some(ValueTy::U8),
                        Keyword(U16) => Some(ValueTy::U16),
                        Keyword(U32) => Some(ValueTy::U32),
                        Keyword(U64) => Some(ValueTy::U64),
                        Keyword(U128) => Some(ValueTy::U128),
                        _ => None,
                    };
                    let skip_cast = target_ty
                        .and_then(|ty| Self::literal_value_type(&*left).filter(|lit| *lit == ty))
                        .is_some();
                    macro_rules! cuto {($inst: expr) => { 
                        Box::new(IRNodeSingle{hrtv, inst: $inst, subx: left} )
                    }}
                    if !skip_cast {
                        let v: Box<dyn IRNode> = match nk {
                            Keyword(U8) => cuto!(CU8),
                            Keyword(U16) => cuto!(CU16),
                            Keyword(U32) => cuto!(CU32),
                            Keyword(U64) => cuto!(CU64),
                            Keyword(U128) => cuto!(CU128),
                            Keyword(Bytes) => cuto!(CBUF),
                            Keyword(Address) => {
                                let para = ValueTy::Address as u8;
                                push_single_p1_hr(hrtv, CTO, para, left)
                            }
                            _ => return e,
                        };
                        left = v;
                    }
                }
                Keyword(Is) => {
                    let e = errf!("<is> express format error");
                    let mut nk = next!();
                    let mut is_not = false;
                    if let Keyword(Not) = nk {
                        is_not = true;
                        nk = next!();
                    }
                    let hrtv = true;
                    left.checkretval()?; // must retv
                    let subx = left;
                    let mut res: Box<dyn IRNode> = match nk {
                        Keyword(Nil)     => Box::new(IRNodeSingle{hrtv, subx, inst: TNIL   }),
                        Keyword(List)    => Box::new(IRNodeSingle{hrtv, subx, inst: TLIST  }),
                        Keyword(Map)     => Box::new(IRNodeSingle{hrtv, subx, inst: TMAP  }),
                        Keyword(Bool)    => push_single_p1_hr(hrtv, TIS, ValueTy::Bool    as u8, subx),
                        Keyword(U8)      => push_single_p1_hr(hrtv, TIS, ValueTy::U8      as u8, subx),
                        Keyword(U16)     => push_single_p1_hr(hrtv, TIS, ValueTy::U16     as u8, subx),
                        Keyword(U32)     => push_single_p1_hr(hrtv, TIS, ValueTy::U32     as u8, subx),
                        Keyword(U64)     => push_single_p1_hr(hrtv, TIS, ValueTy::U64     as u8, subx),
                        Keyword(U128)    => push_single_p1_hr(hrtv, TIS, ValueTy::U128    as u8, subx),
                        Keyword(Bytes)   => push_single_p1_hr(hrtv, TIS, ValueTy::Bytes   as u8, subx),
                        Keyword(Address) => push_single_p1_hr(hrtv, TIS, ValueTy::Address as u8, subx),
                        _ => return e
                    };
                    if is_not {
                        res = Box::new(IRNodeSingle{hrtv, inst: NOT, subx: res})
                    }
                    left = res
                }
                Operator(op) if self.check_op => {
                    if op == OpTy::NOT {
                        return errf!("operator ! cannot be binary");
                    }
                    self.idx -= 1;
                    self.check_op = false;
                    let res = self.parse_next_op(left, 0)?;
                    self.check_op = true;
                    left = res;
                }
                Keyword(And) | Keyword(Or) if self.check_op => {
                    self.idx -= 1;
                    self.check_op = false;
                    let res = self.parse_next_op(left, 0)?;
                    self.check_op = true;
                    left = res;
                }
                _ => { self.idx -= 1; break }
            }
        }
        Ok(left)
    }
    fn parse_next_op(&mut self, mut left: Box<dyn IRNode>, min_prec: u8) -> Ret<Box<dyn IRNode>> {
        loop {
            let op = match self.peek_operator() {
                Some(op) if op.level() >= min_prec => op,
                _ => break,
            };
            if op == OpTy::NOT {
                return errf!("operator !/not cannot be binary");
            }
            self.consume_operator();
            let mut right = self.item_must(0)?;
            right = self.parse_next_op(right, op.next_min_prec())?;
            left.checkretval()?;  // must retv
            right.checkretval()?; // must retv
            left = Box::new(IRNodeDouble{hrtv: true, inst: op.bytecode(), subx: left, suby: right});
        }
        Ok(left)
    }

    fn token_to_operator(token: &Token) -> Option<OpTy> {
        match token {
            Token::Operator(op) => Some(*op),
            Token::Keyword(KwTy::And) => Some(OpTy::AND),
            Token::Keyword(KwTy::Or) => Some(OpTy::OR),
            _ => None,
        }
    }

    fn peek_operator(&self) -> Option<OpTy> {
        self.tokens.get(self.idx).and_then(Self::token_to_operator)
    }

    fn consume_operator(&mut self) -> Option<OpTy> {
        if let Some(token) = self.tokens.get(self.idx) {
            if let Some(op) = Self::token_to_operator(token) {
                self.idx += 1;
                return Some(op);
            }
        }
        None
    }

    pub fn item_must(&mut self, jp: usize) -> Ret<Box<dyn IRNode>> {
        self.idx += jp;
        self.with_expect_retval(true, |s| match s.item_may()? {
            Some(n) => Ok(n),
            None => errf!("not match next Syntax node")
        })
    }

    pub fn item_may_list(&mut self, keep_retval: bool) -> Ret<Box<dyn IRNode>> {
        // NOTE: do NOT unwrap single-item blocks here.
        // We must preserve IRBLOCK/IRBLOCKR opcodes so ircode -> fitsh -> ircode
        // can be byte-for-byte stable under any PrintOption settings.
        Ok(Box::new(self.item_may_block(keep_retval)?))
    }
    
    pub fn item_may_block(&mut self, keep_retval: bool) -> Ret<IRNodeArray> { // return type changed
        let inst = Self::opcode_irblock(keep_retval);
        let mut block = IRNodeArray::with_opcode(inst); // was IRNodeArray::with_opcode(inst);
        let end = self.tokens.len() - 1; // trailing synthetic sentinel
        if self.idx >= end {
            return errf!("block format error")
        }
        let nxt = &self.tokens[self.idx];
        let se = match nxt {
            Partition('{') => '}',
            Partition('(') => ')',
            Partition('[') => ']',
            _ => return errf!("block format error")
        };
        self.idx += 1;
        let mut block_err = false;
        let mut closed = false;
        self.with_expect_retval(keep_retval, |s| {
            loop {
                if s.idx >= end {
                    block_err = true;
                    break
                }
                let nxt = &s.tokens[s.idx];
                if let Partition(sp) = nxt {
                    if *sp == se {
                        closed = true;
                        s.idx += 1;
                        break
                    }else if matches!(sp, '}'|')'|']') {
                        block_err = true;
                        break
                    }
                }
                let Some(li) = s.item_may()? else {
                    break
                };
                block.push( li );
            }
            Ok::<(), Error>(())
        })?;
        if block_err || !closed {
            return errf!("block format error")
        }
        if keep_retval {
            match block.subs.last() {
                None => return errf!("block expression cannot be empty"),
                Some(last) if !last.hasretval() => return errf!("block expression must return value"),
                _ => {},
            }
        }
        Ok(block)
    }

    pub fn item_param(&mut self) -> Ret<Box<dyn IRNode>> {
        let e = errf!("param format error");
        let end = self.tokens.len() - 1; // trailing synthetic sentinel
        if self.idx >= end {
            return e;
        }
        let mut nxt = self.next()?;
        if let Partition('{')= nxt {} else {
            return e
        };
        let mut params = 0;
        let mut param_names = Vec::new();
        loop {
            if self.idx >= end {
                return e;
            }
            nxt = self.next()?;
            match nxt {
                Partition('}') => break, // all finish
                Identifier(id) => {
                    if params == u8::MAX {
                        return errf!("param index overflow");
                    }
                    let name = id.clone();
                    self.bind_local(id, params, SlotKind::Param)?;
                    param_names.push(name);
                    params += 1;
                }
                _ => return e,
            }
        }
        // match
        use Bytecode::*;
        if params == 0 {
            return errf!("param must need at least one");
        }
        self.source_map.register_param_names(param_names)?;
        if params == 1 {
            // Single param: PUT 0 PICK0 (PICK0 moves top to top, PUT consumes; no POP needed)
            Ok(push_single_p1(PUT, 0, push_inst(PICK0)))
        } else {
            // Multi param: UPLIST(PICK0, P0) (caller pushes list)
            Ok(push_double(UPLIST, PICK0, P0))
        }
    }

    fn deal_func_argv(&mut self) -> Ret<Box<dyn IRNode>> {
        let (pms, mut subx) = self.must_get_func_argv(ArgvMode::List)?;
        if 0 == pms {
            // func() == func(nil)
            subx = push_nil()
        }
        Ok(subx)
    }

    pub fn item_identifier(&mut self, id: String) -> Ret<Box<dyn IRNode>> {
        use Bytecode::*;
        use KwTy::*;
        let max = self.tokens.len() - 1;
        // let e0 = errf!("not find identifier '{}'", id);
        let e1 = errf!("call express after identifier format error");
        macro_rules! next {() => {{
            self.idx += 1;
            if self.idx >= max {
                return e1
            }
            &self.tokens[self.idx]
        }}}           
        if start_with_char(&id, '$') {
            let stripped = id.trim_start_matches('$');
            if stripped == "param" {
                return self.item_param();
            }
            if let Some(idx) = Self::parse_slot_str(&id) {
                return Ok(push_local_get(idx, id.clone()));
            }
        }
        if self.idx < max {
            let mut nxt = &self.tokens[self.idx];
            if let Partition('(') = nxt { // function call
                return self.item_func_call(id)
            } else if let Keyword(Dot) = nxt {
                    nxt = next!();
                    let Identifier(func) = nxt else {
                        return e1
                    };
                    self.idx += 1;
                    let fnsg = calc_func_sign(&func);
                    self.source_map.register_func(fnsg, func.clone())?;
                    let fnpm = self.deal_func_argv()?;
                    if id == "this" || id == "self" || id == "super" {
                        let inst = match id.as_str() {
                            "this" => CALLTHIS,
                            "self" => CALLSELF,
                            "super" => CALLSUPER,
                            _ => unreachable!(),
                        };
                        let para: Vec<u8> = fnsg.to_vec(); // fnsig
                        return Ok(Box::new(IRNodeParamsSingle{hrtv: true, inst, para, subx: fnpm}))
                    }
                    // CALL
                    let libi = self.link_lib(&id)?;
                    let para: Vec<u8> = iter::once(libi).chain(fnsg).collect();
                    return Ok(Box::new(IRNodeParamsSingle{hrtv: true, inst: CALL, para,  subx: fnpm}))
                }else if Keyword(Colon) == *nxt || Keyword(DColon) == *nxt {
                    let is_static = Keyword(DColon) == *nxt;
                    nxt = next!();
                    let Identifier(func) = nxt else {
                        return e1
                    };
                    self.idx += 1;
                    let fnsg = calc_func_sign(&func);
                    self.source_map.register_func(fnsg, func.clone())?;
                    let fnpm = self.deal_func_argv()?;
                    let inst = maybe!(is_static, CALLPURE, CALLVIEW);
                    let libi = self.link_lib(&id)?;
                    let para: Vec<u8> = iter::once(libi).chain(fnsg).collect();
                    return Ok(Box::new(IRNodeParamsSingle{hrtv: true, inst, para, subx: fnpm}))
                }
        }
        self.link_symbol(&id)
    }

    pub fn item_may(&mut self) -> Ret<Option<Box<dyn IRNode>>> {
        use Bytecode::*;
        use KwTy::*;
        let max = self.tokens.len() - 1;
        if self.idx >= max {
            return Ok(None) // end
        }
        macro_rules! next { () => {{
            if self.idx >= max {
                return errf!("item_may get next token error")
            }
            let nxt = &self.tokens[self.idx];
            self.idx += 1;
            nxt
        }}}
        let mut nxt = next!();
        let mut item: Box<dyn IRNode> = match nxt {
            Identifier(id) => self.item_identifier(id.clone())?,
            Keyword(This) => self.item_identifier("this".to_string())?,
            Keyword(Self_) => self.item_identifier("self".to_string())?,
            Keyword(Super) => self.item_identifier("super".to_string())?,
            Integer(n) => push_num(*n),
            Token::Address(a) => push_addr(*a),
            Token::Bytes(b) => push_bytes(b)?,
            Partition('(') => {
                let ckop = self.check_op;
                self.check_op = true;
                let exp = self.item_must(0)?;
                self.check_op = ckop; // recover
                exp.checkretval()?; // must retv
                let e = errf!("(..) expression format error");
                nxt = next!();
                let Partition(')') = nxt else {
                    return e
                };
                Box::new(IRNodeWrapOne{node: exp})
            }
            Partition('[') => {
                let mut subs = vec![];
                loop {
                    nxt = next!();
                    if let Partition(']') = nxt {
                        break
                    };
                    self.idx -= 1;
                    let item = self.item_must(0)?;
                    item.checkretval()?; // must retv
                    subs.push(item);
                }
                self.build_list_node(subs)?
            }
            Partition('{') => {
                self.idx -= 1;
                Box::new(self.item_may_block(self.expect_retval)?)
            }
            Token::Operator(op) => match op {
                OpTy::NOT => {
                    let expr = self.item_must(0)?;
                    expr.checkretval()?; // must retv
                    push_single(NOT, expr)
                }
                _ => return errf!("operator {:?} cannot start expression", op)
            },
            Keyword(Not) => {
                let expr = self.item_must(0)?;
                expr.checkretval()?; // must retv
                push_single(NOT, expr)
            }
            Keyword(While) => {
                let exp = self.item_must(0)?;
                exp.checkretval()?; // must retv
                // let e = errf!("while statement format error");
                let suby = self.item_may_list(false)?;
                push_double_box(IRWHILE, exp, suby)
            }
            Keyword(If) => {
                let exp = self.item_must(0)?;
                exp.checkretval()?; // must retv
                let keep_retval = self.expect_retval;
                let list = self.item_may_list(keep_retval)?;
                let mut ifobj = IRNodeTriple{
                    hrtv: keep_retval,
                    inst: Self::opcode_irif(keep_retval),
                    subx: exp,
                    suby: list,
                    subz: IRNodeLeaf::nop_box(),
                };
                let nxt = &self.tokens[self.idx];
                let Keyword(Else) = nxt else {
                    // no else
                    if keep_retval {
                        return errf!("if expression must have else branch")
                    }
                    return Ok(Some(Box::new(ifobj)))
                };
                self.idx += 1; // over else token
                let nxt = &self.tokens[self.idx];
                // else
                let Keyword(If) = nxt else {
                    let elseobj = self.item_may_list(keep_retval)?;
                    ifobj.subz = elseobj;
                    return Ok(Some(Box::new(ifobj)))
                };
                // else if
                let elseifobj = self.with_expect_retval(keep_retval, |s| match s.item_may()? {
                    Some(n) => Ok(n),
                    None => errf!("else if statement format error"),
                })?;
                ifobj.subz = elseifobj;
                Box::new(ifobj)
            }
            Keyword(KwTy::Const) => {
                let e = errf!("const statement format error");
                let token = self.next()?;
                let Identifier(name) = token else {
                    return e
                };
                let token = self.next()?;
                let Keyword(KwTy::Assign) = token else {
                    return e
                };
                let val_token = self.next()?;
                let val_node: Box<dyn IRNode> = match &val_token {
                    Token::Integer(n) => push_num(*n),
                    Token::Bytes(b) => push_bytes(b)?,
                    Token::Address(a) => push_addr(*a),
                    _ => return e,
                };
                let val_str = match val_token {
                    Token::Integer(n) => n.to_string(),
                    Token::Bytes(b) => {
                        if let Ok(s) = String::from_utf8(b.clone()) {
                            format!("\"{}\"", s.escape_default())
                        } else {
                            format!("0x{}", hex::encode(b))
                        }
                    },
                    Token::Address(a) => a.to_readable(),
                    _ => unreachable!(),
                };
                if self.symbols.contains_key(&name) {
                    return errf!("symbol '{}' already defined", name)
                }
                self.symbols.insert(name.clone(), SymbolEntry::Const(dyn_clone::clone_box(val_node.as_ref())));
                self.source_map.register_const(name, val_str)?;
                return Ok(Some(push_empty()));
            }
            Keyword(KwTy::Var) | Keyword(KwTy::Let) => {
                let kind = match nxt {
                    Keyword(KwTy::Var) => SlotKind::Var,
                    Keyword(KwTy::Let) => SlotKind::Let,
                    _ => unreachable!(),
                };
                let err_msg = match kind {
                    SlotKind::Var => "var statement format error",
                    SlotKind::Let => "let statement format error",
                    _ => unreachable!(),
                };
                self.parse_local_statement(kind, err_msg)?
            },
            Keyword(Bind) => {
                let e = errf!("bind statement format error");
                let token = self.next()?;
                let Identifier(name) = token else {
                    return e
                };
                let token = self.next()?;
                let Keyword(KwTy::Assign) = token else {
                    return e
                };
                let expr = self.item_must(0)?;
                expr.checkretval()?; // must retv
                self.bind_macro(name.clone(), expr)?;
                return Ok(Some(push_empty()));
            }
            /*
            Keyword(Use) => { // use AnySwap = emqjNS9PscqdBpMtnC3Jfuc4mvZUPYTPS
                let e = errf!("use statement format error");
                nxt = next!();
                let Identifier(id) = nxt else {
                    return e
                };
                nxt = next!();
                let Keyword(KwTy::Assign) = nxt else {
                    return e
                };
                nxt = next!();
                let Token::Bytes(addr) = nxt else {
                    return e
                };
                self.bind_uses(id.clone(), addr.clone())?;
                push_empty()
            }
            */
            Keyword(Lib) => { // lib AnySwap = 1 : emqjNS9PscqdBpMtnC3Jfuc4mvZUPYTPS
                let e = errf!("lib statement format error");
                nxt = next!();
                let Identifier(id) = nxt else { return e };
                nxt = next!();
                let Keyword(KwTy::Assign) = nxt else { return e };
                nxt = next!();
                let Integer(idx) = nxt else { return e };
                let mut adr = None;
                if self.idx < max && matches!(self.tokens[self.idx], Keyword(Colon)) {
                    self.idx += 1; // consume ':'
                    nxt = next!();
                    let Token::Address(a) = nxt else { return e };
                    adr = Some(*a as field::Address);
                }
                if *idx > u8::MAX as u128 {
                    return errf!("lib statement link index overflow")
                }
                self.bind_lib(id.clone(), *idx as u8, adr)?;
                push_empty()
            }
            Keyword(Param) => {
                self.item_param()?
            }
            Keyword(CallCode) => {
                let e = errf!("callcode statement format error");
                let idx_token = next!();
                let lib_idx = match idx_token {
                    Identifier(id) => self.link_lib(id)?,
                    _ => Self::parse_lib_index_token(idx_token)?,
                };
                nxt = next!();
                let Keyword(DColon) = nxt else {
                    return e
                };
                nxt = next!();
                let fnsg = match nxt {
                    Identifier(func) => {
                        match Self::parse_fn_sig_str(&func) {
                            Ok(sig) => sig,
                            Err(_) => calc_func_sign(&func),
                        }
                    }
                    _ => Self::parse_fn_sig_token(nxt)?,
                };
                let para: Vec<u8> = iter::once(lib_idx).chain(fnsg).collect();
                Box::new(IRNodeParams{hrtv: false, inst: CALLCODE, para})
            }
            Keyword(Call) => {
                let e = errf!("call format error");
                let idx_token = next!();
                let lib_idx = match idx_token {
                    Identifier(id) => self.link_lib(id)?,
                    _ => Self::parse_lib_index_token(idx_token)?,
                };
                nxt = next!();
                let Keyword(DColon) = nxt else {
                    return e
                };
                nxt = next!();
                let sig = Self::parse_fn_sig_token(nxt)?;
                let mut para = Vec::with_capacity(1 + sig.len());
                para.push(lib_idx);
                para.extend(sig);
                let argv = self.deal_func_argv()?;
                Box::new(IRNodeParamsSingle{hrtv: true, inst: CALL, para, subx: argv})
            }
            Keyword(CallView) => {
                let e = errf!("callview format error");
                let idx_token = next!();
                let lib_idx = match idx_token {
                    Identifier(id) => self.link_lib(id)?,
                    _ => Self::parse_lib_index_token(idx_token)?,
                };
                nxt = next!();
                let Keyword(DColon) = nxt else {
                    return e
                };
                nxt = next!();
                let sig = Self::parse_fn_sig_token(nxt)?;
                let mut para = Vec::with_capacity(1 + sig.len());
                para.push(lib_idx);
                para.extend(sig);
                let argv = self.deal_func_argv()?;
                Box::new(IRNodeParamsSingle{hrtv: true, inst: CALLVIEW, para, subx: argv})
            }
            Keyword(CallPure) => {
                let e = errf!("callpure format error");
                let idx_token = next!();
                let lib_idx = match idx_token {
                    Identifier(id) => self.link_lib(id)?,
                    _ => Self::parse_lib_index_token(idx_token)?,
                };
                nxt = next!();
                let Keyword(DColon) = nxt else {
                    return e
                };
                nxt = next!();
                let sig = Self::parse_fn_sig_token(nxt)?;
                let mut para = Vec::with_capacity(1 + sig.len());
                para.push(lib_idx);
                para.extend(sig);
                let argv = self.deal_func_argv()?;
                Box::new(IRNodeParamsSingle{hrtv: true, inst: CALLPURE, para, subx: argv})
            }
            Keyword(CallThis) | Keyword(CallSelf) | Keyword(CallSuper) => {
                let (inst, e) = match nxt {
                    Keyword(CallThis) => (CALLTHIS, errf!("callthis format error")),
                    Keyword(CallSelf) => (CALLSELF, errf!("callself format error")),
                    Keyword(CallSuper) => (CALLSUPER, errf!("callsuper format error")),
                    _ => unreachable!(),
                };
                let idx_token = next!();
                let lib_idx = Self::parse_lib_index_token(idx_token)?;
                if lib_idx != 0 {
                    return e
                }
                nxt = next!();
                let Keyword(DColon) = nxt else {
                    return e
                };
                nxt = next!();
                let sig = Self::parse_fn_sig_token(nxt)?;
                let argv = self.deal_func_argv()?;
                Box::new(IRNodeParamsSingle{hrtv: true, inst, para: sig.to_vec(), subx: argv})
            }
            Keyword(ByteCode) => {
                let e = errf!("bytecode format error");
                nxt = next!();
                let Partition('{') = nxt else {
                    return e
                };
                let mut codes: Vec<u8> = Vec::new();
                loop {
                    let inst: u8;
                    match next!() {
                        Identifier(id) => {
                            let Some(t) = Bytecode::parse(id) else {
                                return errf!("bytecode {} not find", id)
                            };
                            inst = t as u8;
                        }
                        Integer(n) if *n <= u8::MAX as u128 => {
                            inst = *n as u8;
                        }
                        Partition('}') => break, // end
                        _ => return e
                    }
                    codes.push(inst as u8);
                }
                Box::new(IRNodeBytecodes{codes})
            },

            Keyword(List) => {
                let e = errf!("list statement format error");
                nxt = next!();
                let Partition('{') = nxt else { return e };
                let mut subs = vec![];
                loop {
                    nxt = next!();
                    if let Partition('}') = nxt {
                        break
                    };
                    self.idx -= 1;
                    let item = self.item_must(0)?;
                    item.checkretval()?; // must retv
                    subs.push(item);
                }
                self.build_list_node(subs)?
            }
            Keyword(Map) => {
                let e = errf!("map format error");
                nxt = next!();
                let Partition('{') = nxt else {
                    return e
                };
                let mut subs = Vec::new();
                loop {
                    nxt = next!();
                    if let Partition('}') = nxt {
                        break
                    } else {
                        self.idx -= 1;
                    }
                    let Some(k) = self.item_may()? else {
                        break
                    };
                    k.checkretval()?;
                    nxt = next!();
                    let Keyword(Colon) = nxt else {
                        return e
                    };
                    let Some(v) = self.item_may()? else {
                        return e
                    };
                    v.checkretval()?;
                    subs.push(k);
                    subs.push(v);
                }
                let num = subs.len();
                if num == 0 {
                    push_inst(NEWMAP)
                } else {
                    // PACKMAP expects total item count (k+v pairs), not pair count
                    subs.push(push_num(num as u128));
                    subs.push(push_inst(PACKMAP));
                    let arys = Self::build_irlist(subs)?; // changed
                    Box::new(arys)
                }
            }
            Keyword(Log) => {
                let e = errf!("log argv number error");
                // `log` consumes values from the stack (see interpreter: LOG1 pops 2, LOG2 pops 3, ...).
                // Therefore log arguments must be parsed as value expressions.
                let max = self.tokens.len() - 1;
                if self.idx >= max {
                    return e
                }
                let (open, close) = match &self.tokens[self.idx] {
                    Partition('(') => ('(', ')'),
                    Partition('{') => ('{', '}'),
                    Partition('[') => ('[', ']'),
                    _ => return e,
                };
                let mut subs = Self::parse_delimited_value_exprs(self, open, close, "log argv number error")?;

                let num = subs.len();
                match num {
                    2 | 3 | 4 | 5 => {
                        let inst = match num {
                            2 => LOG1,
                            3 => LOG2,
                            4 => LOG3,
                            5 => LOG4,
                            _ => never!(),
                        };
                        subs.push(push_inst_noret(inst));
                        let arys = Self::build_irlist(subs)?; // changed
                        Box::new(arys)
                    }
                    _ => return e,
                }
            }
            Keyword(Nil)    => push_nil(),
            Keyword(True)   => push_inst(PTRUE),
            Keyword(False)  => push_inst(PFALSE),
            Keyword(Abort)  => push_inst_noret(ABT),
            Keyword(End)    => push_inst_noret(END),
            Keyword(Print)  => {
                let exp = self.item_must(0)?;
                if !exp.hasretval() {
                    return errf!("print arguments must be expressions with return values; do not use bind/var declarations directly");
                }
                push_single_noret(PRT, exp)
            },
            Keyword(Assert) => {
                let exp = self.item_must(0)?;
                if !exp.hasretval() {
                    return errf!("assert arguments must be expressions with return values");
                }
                push_single_noret(AST, exp)
            }
            Keyword(Throw)  => {
                let exp = self.item_must(0)?;
                if !exp.hasretval() {
                    return errf!("throw arguments must be expressions with return values");
                }
                push_single_noret(ERR, exp)
            }
            Keyword(Return) => {
                let exp = self.item_must(0)?;
                if !exp.hasretval() {
                    return errf!("return arguments must be expressions with return values");
                }
                push_single_noret(RET, exp)
            }
            _ => return errf!("unsupport token '{:?}'", nxt),
        };
        item = self.item_with_left(item)?;
        Ok(Some(item))
    }


    pub fn with_params(mut self, params: Vec<(String, ValueTy)>) -> Self {
        self.ext_params = Some(params);
        self
    }

    pub fn with_libs(mut self, libs: Vec<(String, u8, Option<field::Address>)>) -> Self {
        self.ext_libs = Some(libs);
        self
    }

    pub fn with_ircode(mut self, is_ircode: bool) -> Self {
        self.is_ircode = is_ircode;
        self
    }

    pub fn parse(mut self) -> Ret<(IRNodeArray, SourceMap)> {
        use Bytecode::*;
        // reserve head for ALLOC
        self.irnode.push(push_empty());

        // External Libs
        if let Some(libs) = self.ext_libs.take() {
            for (name, idx, addr) in libs {
                self.bind_lib(name, idx, addr)?;
            }
        }
        // External Params
        if let Some(params) = self.ext_params.take() {
            let mut param_names = Vec::new();
            for (i, (name, _ty)) in params.iter().enumerate() {
                if i > u8::MAX as usize {
                    return errf!("param index {} overflow", i);
                }
                let idx = i as u8;
                self.bind_local(name.clone(), idx, SlotKind::Var)?;
                param_names.push(name.clone());
            }
            if !param_names.is_empty() {
                self.source_map.register_param_names(param_names)?;
            }

            match params.len() {
                0 => {
                    // pop implicit nil argument (auto-inserted by caller)
                    self.irnode.push(push_inst_noret(POP));
                }
                1 => {
                    // Single param: PUT 0 PICK0 (PICK0 moves top to top, PUT consumes; no POP needed)
                    self.irnode.push(push_single_p1(PUT, 0, push_inst(PICK0)));
                }
                _ => {
                    // Multi param: UPLIST(PICK0, P0) (caller pushes list)
                    let unpack = push_double(UPLIST, PICK0, P0);
                    self.irnode.push(unpack);
                }
            }
        }

        while let Some(item) = self.item_may()? {
            if let Some(..) = item.as_any().downcast_ref::<IRNodeEmpty>() {} else {
                self.irnode.push(item);
            };
        }
        let subs = &mut self.irnode.subs;
        if self.local_alloc > 0 {
            let allocs = Box::new(IRNodeParam1 {
                hrtv: false,
                inst: ALLOC,
                para: self.local_alloc,
                text: s!(""),
            });
            let mut exist = false;
            if subs.len() > 1 && subs[1].bytecode() == ALLOC as u8 {
                exist = true;
            }
            if exist {
                subs[1] = allocs;
            } else {
                subs[0] = allocs;
            }
        }
        let block = self.irnode;
        let source_map = self.source_map;
        Ok((block, source_map))
    }


}
