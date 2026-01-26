use std::collections::{HashMap, HashSet};
use std::convert::TryInto;
use hex;

enum SymbolEntry {
    Slot(u8, bool),
    Bind(Box<dyn IRNode>),
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
    // leftv: Box<dyn AST>,
    irnode: IRNodeArray, // replaced IRNodeArray -> IRNodeArray
    source_map: SourceMap,
}


#[allow(dead_code)]
impl Syntax {
    fn build_packlist_node(&mut self, mut subs: Vec<Box<dyn IRNode>>) -> Ret<Box<dyn IRNode>> {
        let num = subs.len();
        if num == 0 {
            return Ok(push_inst(Bytecode::NEWLIST));
        }
        subs.push(push_num(num as u128));
        subs.push(push_inst(Bytecode::PACKLIST));
        let arys = IRNodeArray::from_vec(subs, Bytecode::IRLIST)?;
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

    pub fn bind_local_assign(&mut self, s: String, v: Box<dyn IRNode>, kind: SlotKind) -> Ret<Box<dyn IRNode>> {
        let idx = self.local_alloc;
        self.bind_local_assign_replace(s, idx, v, kind)
    }

    pub fn bind_local_assign_replace(&mut self, s: String, idx: u8, v: Box<dyn IRNode>, kind: SlotKind) -> Ret<Box<dyn IRNode>> {
        use Bytecode::*;
        self.bind_local(s, idx, kind)?;
        Ok(Box::new(IRNodeParam1Single{hrtv: false, inst: PUT, para: idx, subx: v}))
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
            val = Some(self.item_must(0)?);
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
        match self.symbols.get(s) {
            Some(SymbolEntry::Slot(i, _)) => Ok(push_local_get(*i, text)),
            _ => return errf!("cannot find symbol '{}'", s),
        }
    }

    fn slot_info(&self, s: &String) -> Ret<(u8, bool)> {
        match self.symbols.get(s) {
            Some(SymbolEntry::Slot(idx, mutable)) => Ok((*idx, *mutable)),
            _ => errf!("cannot find symbol '{}'", s),
        }
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
            Some(SymbolEntry::Slot(_, _)) => self.link_local(s),
            None => errf!("cannot find symbol '{}'", s),
        }
    }

    pub fn save_local(&mut self, s: &String, v: Box<dyn IRNode>) -> Ret<Box<dyn IRNode>> {
        use Bytecode::*;
        let (i, mutable) = self.slot_info(s)?;
        if !mutable {
            return errf!("cannot assign to immutable symbol '{}'", s)
        }
        self.source_map.mark_slot_mutated(i);
        Ok(Box::new(IRNodeParam1Single{hrtv: false, inst: PUT, para: i, subx: v }))
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
            return Ok(Box::new(IRNodeParam1Single{hrtv: false, inst: XOP, para: mark, subx: v }))
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
        Ok(Box::new(IRNodeParam1Single{hrtv: false, inst: PUT, para: i, subx: opsv }))
    }
    

    pub fn new(mut tokens: Vec<Token>) -> Self {
        tokens.push(Token::Partition('}'));
        Self {
            tokens,
            irnode: IRNodeArray::with_opcode(Bytecode::IRBLOCK), // was IRNodeArray::new()
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
                    macro_rules! cuto {($inst: expr) => { 
                        Box::new(IRNodeSingle{hrtv, inst: $inst, subx: left} )
                    }}
                    let v: Box<dyn IRNode> = match nk {
                        Keyword(U8)    => cuto!(CU8)  ,
                        Keyword(U16)   => cuto!(CU16) ,
                        Keyword(U32)   => cuto!(CU32) ,
                        Keyword(U64)   => cuto!(CU64) ,
                        Keyword(U128)  => cuto!(CU128),
                        Keyword(Bytes) => cuto!(CBUF) ,
                        Keyword(Address) => {
                            let para = ValueTy::Address as u8;
                            Box::new(IRNodeParam1Single{hrtv, inst: CTO, para, subx: left })       
                        }
                        _ => return e
                    };
                    left = v;
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
                    let subx = left;
                    let mut res: Box<dyn IRNode> = match nk {
                        Keyword(Nil)     => Box::new(IRNodeSingle{hrtv, subx, inst: TNIL   }),
                        Keyword(List)    => Box::new(IRNodeSingle{hrtv, subx, inst: TLIST  }),
                        Keyword(Map)     => Box::new(IRNodeSingle{hrtv, subx, inst: TMAP  }),
                        Keyword(Bool)    => Box::new(IRNodeParam1Single{para: ValueTy::Bool    as u8, hrtv, subx, inst: TIS}),
                        Keyword(U8)      => Box::new(IRNodeParam1Single{para: ValueTy::U8      as u8, hrtv, subx, inst: TIS}),
                        Keyword(U16)     => Box::new(IRNodeParam1Single{para: ValueTy::U16     as u8, hrtv, subx, inst: TIS}),
                        Keyword(U32)     => Box::new(IRNodeParam1Single{para: ValueTy::U32     as u8, hrtv, subx, inst: TIS}),
                        Keyword(U64)     => Box::new(IRNodeParam1Single{para: ValueTy::U64     as u8, hrtv, subx, inst: TIS}),
                        Keyword(U128)    => Box::new(IRNodeParam1Single{para: ValueTy::U128    as u8, hrtv, subx, inst: TIS}),
                        Keyword(Bytes)   => Box::new(IRNodeParam1Single{para: ValueTy::Bytes   as u8, hrtv, subx, inst: TIS}),
                        Keyword(Address) => Box::new(IRNodeParam1Single{para: ValueTy::Address as u8, hrtv, subx, inst: TIS}),
                        _ => return e
                    };
                    if is_not {
                        res = Box::new(IRNodeSingle{hrtv, inst: NOT, subx: res})
                    }
                    left = res
                }
                Operator(_) if self.check_op => {
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
            self.consume_operator();
            let mut right = self.item_must(0)?;
            right = self.parse_next_op(right, op.next_min_prec())?;
            left = Box::new(IRNodeDouble{hrtv: true, inst: op.bytecode(), subx: left, suby: right});
        }
        Ok(left)
    }

    fn peek_operator(&self) -> Option<OpTy> {
        self.tokens.get(self.idx).and_then(|token| {
            if let Token::Operator(op) = token {
                Some(*op)
            } else {
                None
            }
        })
    }

    fn consume_operator(&mut self) -> Option<OpTy> {
        if let Some(token) = self.tokens.get(self.idx) {
            if let Token::Operator(op) = token {
                self.idx += 1;
                return Some(*op);
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
        let block = self.item_may_block(keep_retval)?;
        Ok(match block.len() {
            0 => push_empty(),
            1 => block.into_one(),
            _ => Box::new(block)
        })
    }
    
    pub fn item_may_block(&mut self, keep_retval: bool) -> Ret<IRNodeArray> { // return type changed
        use Bytecode::*;
        let inst = if keep_retval {
            IRBLOCKR
        } else {
            IRBLOCK
        };
        let mut block = IRNodeArray::with_opcode(inst); // was IRNodeArray::with_opcode(inst);
        let max = self.tokens.len() - 1;
        if self.idx >= max {
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
        self.with_expect_retval(keep_retval, |s| {
            loop {
                if s.idx >= max { break }
                let nxt = &s.tokens[s.idx];
                if let Partition(sp) = nxt {
                    if *sp == se {
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
        if block_err {
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
        let mut nxt = self.next()?;
        if let Partition('{')= nxt {} else {
            return e
        };
        let mut params = 0;
        let mut param_names = Vec::new();
        loop {
            nxt = self.next()?;
            if let Partition('}') = nxt {
                break // all finish
            };
            if let Identifier(id) = nxt {
                let name = id.clone();
                self.bind_local(id, params, SlotKind::Param)?;
                param_names.push(name);
                params += 1;
            } 
        }
        // match
        use Bytecode::*;
        if params == 0 {
            return errf!("param must need at least one");
        }
        self.source_map.register_param_names(param_names)?;
        // var num = pick(0)
        Ok(match params {
            1 => Box::new(IRNodeParam1Single{
                hrtv: true,
                inst: PUT,
                para: 0,
                subx: Box::new(IRNodeParam1{
                    hrtv: true,
                    inst: PICK,
                    para: 0,
                    text: s!("")
                })
            }),
            // unpack list
            _ => Box::new(IRNodeDouble{
                hrtv: true, 
                inst: UPLIST,
                subx: push_inst(Bytecode::DUP),
                suby: push_inst(Bytecode::P0),
            })
        })
    }

    fn deal_func_argv(&mut self) -> Ret<Box<dyn IRNode>> {
        let (pms, mut subx) = self.must_get_func_argv(ArgvMode::PackList)?;
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
            } else if let Partition('[') = nxt { // item get
                // println!("---------item_identifier---------- self.tokens[self.idx]= {:?}", nxt); 
                return self.item_get(id)
                } else if let Keyword(Dot) = nxt {
                    nxt = next!();
                    let Identifier(func) = nxt else {
                        return e1
                    };
                    self.idx += 1;
                    let fnsg = calc_func_sign(&func);
                    self.source_map.register_func(fnsg, func.clone())?;
                    let fnpm = self.deal_func_argv()?;
                    return Ok(match &id=="self" {
                        true => { // CALLINR
                            let para: Vec<u8> = fnsg.to_vec(); // fnsig
                            Box::new(IRNodeParamsSingle{hrtv: true, inst: CALLINR, para, subx: fnpm})
                        },
                        false => { // CALL
                            let libi = self.link_lib(&id)?;
                            let para: Vec<u8> = iter::once(libi).chain(fnsg).collect();
                            Box::new(IRNodeParamsSingle{hrtv: true, inst: CALL, para,  subx: fnpm})
                        },
                    })
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
                    let inst = maybe!(is_static, CALLSTATIC, CALLLIB);
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
        macro_rules! back { () => {
            self.idx -= 1;
        }}
        let mut nxt = next!();
        let mut item: Box<dyn IRNode> = match nxt {
            Identifier(id) => self.item_identifier(id.clone())?,
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
                self.build_packlist_node(subs)?
            }
            Partition('{') => {
                self.idx -= 1;
                Box::new(self.item_may_block(self.expect_retval)?)
            }
            Token::Operator(op) => match op {
                OpTy::NOT => {
                    let expr = self.item_must(0)?;
                    expr.checkretval()?; // must retv
                    Box::new(IRNodeSingle{hrtv: true, inst: NOT, subx: expr})
                }
                _ => return errf!("operator {:?} cannot start expression", op)
            },
            Keyword(While) => {
                let exp = self.item_must(0)?;
                exp.checkretval()?; // must retv
                // let e = errf!("while statement format error");
                let suby = self.item_may_list(false)?;
                Box::new(IRNodeDouble{hrtv: false, inst: IRWHILE, subx: exp, suby})
            }
            Keyword(If) => {
                let exp = self.item_must(0)?;
                exp.checkretval()?; // must retv
                let keep_retval = self.expect_retval;
                let list = self.item_may_list(keep_retval)?;
                let mut ifobj = IRNodeTriple{
                    hrtv: keep_retval, inst: maybe!(keep_retval, IRIFR, IRIF), subx: exp, suby: list, subz: IRNodeLeaf::nop_box()
                };
                let nxt = &self.tokens[self.idx];
                let Keyword(Else) = nxt else {
                    // no else
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
                let elseifobj = self.item_must(0)?;
                ifobj.subz = elseifobj;
                Box::new(ifobj)
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
                nxt = next!();
                let mut adr = None;
                if let Keyword(Colon) = nxt {
                    nxt = next!();
                    let Token::Address(a) = nxt else { return e };
                    adr = Some(*a as field::Address);
                }else{
                    back!();
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
                let lib_idx = Self::parse_lib_index_token(idx_token)?;
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
            Keyword(CallLib) => {
                let e = errf!("calllib format error");
                let idx_token = next!();
                let lib_idx = Self::parse_lib_index_token(idx_token)?;
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
                Box::new(IRNodeParamsSingle{hrtv: true, inst: CALLLIB, para, subx: argv})
            }
            Keyword(CallStatic) => {
                let e = errf!("callstatic format error");
                let idx_token = next!();
                let lib_idx = Self::parse_lib_index_token(idx_token)?;
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
                Box::new(IRNodeParamsSingle{hrtv: true, inst: CALLSTATIC, para, subx: argv})
            }
            Keyword(CallInr) => {
                nxt = next!();
                let sig = Self::parse_fn_sig_token(nxt)?;
                let argv = self.deal_func_argv()?;
                Box::new(IRNodeParamsSingle{hrtv: true, inst: CALLINR, para: sig.to_vec(), subx: argv})
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

            Keyword(Packlist) => {
                let e = errf!("packlist statement format error");
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
                self.build_packlist_node(subs)?
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
                subs.push(push_num(subs.len() as u128));
                subs.push(push_inst(PACKMAP));
                let arys = IRNodeArray::from_vec(subs, Bytecode::IRLIST)?; // changed
                Box::new(arys)
            }
            Keyword(Log) => {
                let e = errf!("log argv number error");
                let block = self.item_may_block(false)?;
                let num = block.subs.len();
                match num {
                    2 | 3 | 4 | 5 => {
                        let inst = match num {
                            2 => LOG1,
                            3 => LOG2,
                            4 => LOG3,
                            5 => LOG4,
                            _ => never!()
                        };
                        let mut subs = block.subs;
                        subs.push(push_inst_noret(inst));
                        let arys = IRNodeArray::from_vec(subs, Bytecode::IRLIST)?; // changed
                        Box::new(arys)
                    }
                    _ => return e
                }
            }
            Keyword(Nil)    => push_nil(),
            Keyword(True)   => push_inst(P1),
            Keyword(False)  => push_inst(P0),
            Keyword(Abort)  => push_inst_noret(ABT),
            Keyword(End)    => push_inst_noret(END),
            Keyword(Print)  => {
                let exp = self.item_must(0)?;
                if !exp.hasretval() {
                    return errf!("print arguments must be expressions with return values; do not use bind/var declarations directly");
                }
                Box::new(IRNodeSingle{hrtv: false, inst: PRT, subx: exp})
            },
            Keyword(Assert) => Box::new(IRNodeSingle{hrtv: false, inst: AST, subx: self.item_must(0)?}),
            Keyword(Throw)  => Box::new(IRNodeSingle{hrtv: false, inst: ERR, subx: self.item_must(0)?}),
            Keyword(Return) => Box::new(IRNodeSingle{hrtv: false, inst: RET, subx: self.item_must(0)?}),
            _ => return errf!("unsupport token '{:?}'", nxt),
        };
        item = self.item_with_left(item)?;
        Ok(Some(item))
    }


    pub fn parse(mut self) -> Ret<(IRNodeArray, SourceMap)> {
        self.irnode.push(push_empty());
        while let Some(item) = self.item_may()? {
            if let Some(..) = item.as_any().downcast_ref::<IRNodeEmpty>() {} else {
                self.irnode.push(item);
            };
        }
        let subs = &mut self.irnode.subs;
        if self.local_alloc > 0 {
            let allocs = Box::new(IRNodeParam1 {
                hrtv: false,
                inst: Bytecode::ALLOC,
                para: self.local_alloc,
                text: s!(""),
            });
            let mut exist = false;
            if subs.len() > 1 && subs[1].bytecode() == Bytecode::ALLOC as u8 {
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
