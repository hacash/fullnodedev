
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;



#[derive(Clone)]
enum SymbolEntry {
    Var(u8),
    Let(Rc<RefCell<LetInfo>>),
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
    irnode: IRNodeBlock,
}


#[allow(dead_code)]
impl Syntax {


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
        self.bdlibs.insert(s, (idx, adr));
        Ok(())
    }

    fn parse_slot_token(token: &Token) -> Option<u8> {
        if let Identifier(id) = token {
            if start_with_char(id, '$') {
                if let Ok(idx) = id.trim_start_matches('$').parse::<u8>() {
                    return Some(idx);
                }
            }
        }
        None
    }

    pub fn bind_local_assign(&mut self, s: String, v: Box<dyn IRNode>) -> Ret<Box<dyn IRNode>> {
        let idx = self.local_alloc;
        self.bind_local_assign_replace(s, idx, v)
    }

    pub fn bind_local_assign_replace(&mut self, s: String, idx: u8, v: Box<dyn IRNode>) -> Ret<Box<dyn IRNode>> {
        use Bytecode::*;
        self.bind_local(s, idx)?;
        Ok(Box::new(IRNodeParam1Single{hrtv: false, inst: PUT, para: idx, subx: v}))
    }

    // ret empty
    pub fn bind_local(&mut self, s: String, idx: u8) -> Ret<Box<dyn IRNode>> {
        self.reserve_slot(idx)?;
        if idx >= self.local_alloc {
            self.local_alloc = idx + 1;
        }
        self.register_symbol(s, SymbolEntry::Var(idx))?;
        Ok(empty())
    }

    fn register_symbol(&mut self, s: String, entry: SymbolEntry) -> Rerr {
        if let Some(..) = self.symbols.get(&s) {
            return errf!("symbol '{}' already bound", s)
        }
        self.symbols.insert(s, entry);
        Ok(())
    }

    pub fn bind_let(&mut self, s: String, v: Box<dyn IRNode>) -> Rerr {
        let info = Rc::new(RefCell::new(LetInfo::new(v)));
        self.register_symbol(s, SymbolEntry::Let(info))?;
        Ok(())
    }

    fn reserve_slot(&mut self, idx: u8) -> Rerr {
        if self.slot_used.contains(&idx) {
            return errf!("slot {} already used", idx)
        }
        self.slot_used.insert(idx);
        Ok(())
    }

    pub fn link_local(&self, s: &String) -> Ret<Box<dyn IRNode>> {
        let text = s.clone();
        match self.symbols.get(s) {
            Some(SymbolEntry::Var(i)) => Ok(push_local_get(*i, text)),
            _ => return errf!("cannot find symbol '{}'", s),
        }
    }

    pub fn link_let(&mut self, s: &String) -> Ret<Box<dyn IRNode>> {
        let info_rc = match self.symbols.get(s) {
            Some(SymbolEntry::Let(info)) => Rc::clone(info),
            _ => return errf!("cannot find or relink symbol '{}'", s),
        };
        let (ref_idx, hrtv) = {
            let mut info = info_rc.borrow_mut();
            let idx = info.refs;
            info.refs = info.refs.saturating_add(1);
            if info.refs == 2 {
                info.needs_slot = true;
            }
            (idx, info.expr.hasretval())
        };
        Ok(Box::new(IRNodeLetRef { info: info_rc, ref_idx, hrtv }))
    }

    pub fn link_symbol(&mut self, s: &String) -> Ret<Box<dyn IRNode>> {
        match self.symbols.get(s) {
            Some(SymbolEntry::Let(_)) => self.link_let(s),
            Some(SymbolEntry::Var(_)) => self.link_local(s),
            None => errf!("cannot find symbol '{}'", s),
        }
    }

    pub fn save_local(&self, s: &String, v: Box<dyn IRNode>) -> Ret<Box<dyn IRNode>> {
        use Bytecode::*;
        match self.symbols.get(s) {
            Some(SymbolEntry::Var(i)) => Ok(Box::new(IRNodeParam1Single{hrtv: false, inst: PUT, para: *i, subx: v })),
            _ => return errf!("cannot find symbol '{}'", s),
        }
    }
    
    pub fn assign_local(&self, s: &String, v: Box<dyn IRNode>, op: &Token) -> Ret<Box<dyn IRNode>> {
        use Bytecode::*;
        use KwTy::*;
        let i = match self.symbols.get(s) {
            Some(SymbolEntry::Var(idx)) => *idx,
            _ => return errf!("cannot find symbol '{}'", s),
        };
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
            irnode: IRNodeBlock::new(),
            check_op: true,
            ..Default::default()
        }
    }


    pub fn item_with_left(&mut self, left: Box<dyn IRNode>) -> Ret<Box<dyn IRNode>> {
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
            let nxt = &self.tokens[self.idx];
            self.idx += 1;
            nxt
        }}}
        let mut nxt = next!();
        Ok(match nxt {
            Keyword(Assign) 
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
                match nxt {
                    Keyword(Assign) => self.save_local(&id, v)?,
                    _ => self.assign_local(&id, v, nxt)?,
                }
            }
            Keyword(As) => {
                left.checkretval()?; // must retv
                let e = errf!("<as> express format error");
                nxt = next!();
                let hrtv = true;
                macro_rules! cuto {($inst: expr) => { 
                    Box::new(IRNodeSingle{hrtv, inst: $inst, subx: left} )
                }}
                let v: Box<dyn IRNode> = match nxt {
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
                v
            }
            Keyword(Is) => {
                let e = errf!("<is> express format error");
                nxt = next!();
                let mut is_not = false;
                if let Keyword(Not) = nxt {
                    is_not = true;
                    nxt = next!();
                }
                let hrtv = true;
                let subx = left;
                let mut res: Box<dyn IRNode> = match nxt {
                    Keyword(Nil)     => Box::new(IRNodeSingle{hrtv, subx, inst: TNIL   }),
                    Keyword(List)    => Box::new(IRNodeSingle{hrtv, subx, inst: TLIST  }),
                    Keyword(Map)     => Box::new(IRNodeSingle{hrtv, subx, inst: TMAP   }),
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
                    res = Box::new(IRNodeSingle{hrtv: true, inst: NOT, subx: res})
                }
                res
            }
            Operator(op) if self.check_op => {
                self.check_op = false;
                let res = self.parse_next_op(left, *op)?;
                self.check_op = true;
                res
            }
            _ => { self.idx -= 1; left }
        })
    }

    fn parse_next_op(&mut self, mut left: Box<dyn IRNode>, op: OpTy) -> Ret<Box<dyn IRNode>> {
        let mut right = self.item_must(0)?;
        let c_node = |op: OpTy, l, r| { Box::new(IRNodeDouble{hrtv: true, inst: op.bytecode(), subx: l, suby: r}) };
        if let Ok(nxt) = self.next() {
            if let Operator(op2) = nxt {
                if op.level() >= op2.level() {
                    left = c_node(op, left, right);
                    return self.parse_next_op(left, op2)
                }else{
                    right = self.parse_next_op(right, op2)?;
                }
            }else{
                self.idx -= 1; // back
            }
        }
        Ok(c_node(op, left, right))
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
            0 => empty(),
            1 => block.into_one(),
            _ => Box::new(block)
        })
    }
    
    pub fn item_may_block(&mut self, keep_retval: bool) -> Ret<IRNodeBlock> {
        use Bytecode::*;
        let inst = if keep_retval {
            IRBLOCKR
        } else {
            IRBLOCK
        };
        let mut block = IRNodeBlock::with_opcode(inst);
        let max = self.tokens.len() - 1;
        let e =  errf!("block format error");
        if self.idx >= max {
            return e
        }
        let nxt = &self.tokens[self.idx];
        let se = match nxt {
            Partition('{') => '}',
            Partition('(') => ')',
            Partition('[') => ']',
            _ => return e
        };
        self.idx += 1;
        loop {
            if self.idx >= max { break }
            let nxt = &self.tokens[self.idx];
            if let Partition(sp) = nxt {
                if *sp == se {
                    self.idx += 1;
                    break
                }else {
                    return e
                }
            }
            let Some(li) = self.item_may()? else {
                break
            };
            block.push( li );
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
        loop {
            nxt = self.next()?;
            if let Partition('}') = nxt {
                break // all finish
            };
            if let Identifier(id) = nxt {
                self.bind_local(id, params)?;
                params += 1;
            } 
        }
        // match
        use Bytecode::*;
        Ok(match params {
            0 => return errf!("param must need at least one"),
            // var num = pick(0)
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
        /* if start_with_char(&id, '$') {
            let k = id.trim_start_matches('$');
            return match k {
                "param" => self.item_param(),
                _ => e0
            }
        } */
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
                let fnsg = calc_func_sign(func);
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
                let fnsg = calc_func_sign(func);
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
            Token::Bytes(b) => item_bytes(b)?,
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
                maybe!(exp.subs() >= 2,
                    Box::new(IRNodeWrapOne{node: exp}),
                    exp
                )
            }
            Partition('[') => { // pack_list
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
                let num = subs.len();
                let mut list = IRNodeList{subs, inst: Bytecode::IRLIST};
                    list.push(push_num(num as u128));
                    list.push(push_inst(Bytecode::PACKLIST));
                Box::new(list)
            }
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
            Keyword(Var) => { // let foo = $0
                let e = errf!("var statement format error");
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
                let Identifier(id) = next!() else {
                    return e
                };
                let vk = id.clone();
                nxt = next!();
                let mut idx = None;
                let mut val = None;
                if let Some(i) = gidx(nxt) {
                    idx = Some(i);
                    nxt = next!();
                }
                if let Keyword(Assign) = nxt {
                    val = Some(self.item_must(0)?);
                } else {
                    self.idx -= 1;
                }
                match (idx, val) {
                    (Some(i), Some(v)) => self.bind_local_assign_replace(vk, i, v),
                    (.., Some(v))      => self.bind_local_assign(vk, v),
                    (Some(i), ..)      => self.bind_local(vk, i),
                    _ => return e
                }?
            }
            Keyword(Let) => { // let name? $slot? = expr
                let e = errf!("let statement format error");
                let mut nxt = next!();
                let mut name: Option<String> = None;
                let mut slot: Option<u8> = None;
                if let Identifier(id) = &nxt {
                    if let Some(idx) = Self::parse_slot_token(&nxt) {
                        slot = Some(idx);
                    } else {
                        name = Some(id.clone());
                        if self.idx < self.tokens.len() {
                            let peek = &self.tokens[self.idx];
                            if let Some(idx) = Self::parse_slot_token(peek) {
                                slot = Some(idx);
                                self.idx += 1;
                            }
                        }
                    }
                } else {
                    return e
                }
                nxt = next!();
                let Keyword(Assign) = nxt else {
                    return e
                };
                let exp = self.item_must(0)?;
                exp.checkretval()?; // must retv
                if let Some(idx) = slot {
                    let sym = name.unwrap_or_else(|| format!("${}", idx));
                    let node = self.bind_local_assign_replace(sym, idx, exp)?;
                    return Ok(Some(node));
                }
                if let Some(n) = name {
                    self.bind_let(n, exp)?;
                    return Ok(Some(Self::empty()));
                }
                return errf!("let statement needs at least a name or slot")
            }
            /*
            Keyword(Use) => { // use AnySwap = VFE6Zu4Wwee1vjEkQLxgVbv3c6Ju9iTaa
                let e = errf!("use statement format error");
                nxt = next!();
                let Identifier(id) = nxt else {
                    return e
                };
                nxt = next!();
                let Keyword(Assign) = nxt else {
                    return e
                };
                nxt = next!();
                let Token::Bytes(addr) = nxt else {
                    return e
                };
                self.bind_uses(id.clone(), addr.clone())?;
                Self::empty()
            }
            */
            Keyword(Lib) => { // lib AnySwap = 1 : VFE6Zu4Wwee1vjEkQLxgVbv3c6Ju9iTaa
                let e = errf!("lib statement format error");
                nxt = next!();
                let Identifier(id) = nxt else { return e };
                nxt = next!();
                let Keyword(Assign) = nxt else { return e };
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
                Self::empty()
            }
            Keyword(Param) => {
                self.item_param()?
            }
            Keyword(CallCode) => {
                let e = errf!("callcode statement format error");
                nxt = next!();
                let Identifier(id) = nxt else {
                    return e
                };
                nxt = next!();
                let Keyword(DColon) = nxt else {
                    return e
                };
                nxt = next!();
                let Identifier(func) = nxt else {
                    return e
                };
                let fnsg = calc_func_sign(func);
                let para: Vec<u8> = iter::once(self.link_lib(id)?).chain(fnsg).collect();
                Box::new(IRNodeParams{hrtv: false, inst: CALLCODE, para})
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
                // let e = errf!("list format error");
                let block = self.item_may_block(false)?;
                let num = block.subs.len();
                match num {
                    0 => Self::push_inst(NEWLIST),
                    _ => {
                        let mut subs = block.subs;
                        subs.push(Self::push_num(num as u128));
                        subs.push(Self::push_inst(PACKLIST));
                        let arys = IRNodeList::from_vec(subs)?;
                        Box::new(arys)
                    }
                }
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
                subs.push(Self::push_num(subs.len() as u128));
                subs.push(Self::push_inst(PACKMAP));
                let arys = IRNodeList::from_vec(subs)?;
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
                        subs.push(Self::push_inst_noret(inst));
                        let arys = IRNodeList::from_vec(subs)?;
                        Box::new(arys)
                    }
                    _ => return e
                }
            }
            Keyword(Nil)    => Self::push_nil(),
            Keyword(True)   => Self::push_inst(P1),
            Keyword(False)  => Self::push_inst(P0),
            Keyword(Abort)  => Self::push_inst_noret(ABT),
            Keyword(End)    => Self::push_inst_noret(END),
            Keyword(Print)  => Box::new(IRNodeSingle{hrtv: false, inst: PRT, subx: self.item_must(0)?}),
            Keyword(Assert) => Box::new(IRNodeSingle{hrtv: false, inst: AST, subx: self.item_must(0)?}),
            Keyword(Throw)  => Box::new(IRNodeSingle{hrtv: false, inst: ERR, subx: self.item_must(0)?}),
            Keyword(Return) => Box::new(IRNodeSingle{hrtv: false, inst: RET, subx: self.item_must(0)?}),
            _ => return errf!("unsupport token '{:?}'", nxt),
        };
        item = self.item_with_left(item)?;
        Ok(Some(item))
    }


    pub fn parse(mut self) -> Ret<IRNodeBlock> {
        // for local alloc
        self.irnode.push(Self::empty());
        // bodys
        while let Some(item) = self.item_may()? {
            if let Some(..) = item.as_any().downcast_ref::<IRNodeEmpty>() {} else {
                self.irnode.push(item);
            };
        }
        self.finalize_let_slots()?;
        // local alloc
        if self.local_alloc > 0 {
            let allocs = Box::new(IRNodeParam1{
                hrtv: false, inst: Bytecode::ALLOC, para: self.local_alloc, text: s!("")
            });
            self.irnode.subs[0] = allocs;
        }else{
            self.irnode.subs.remove(0); // no local var
        }
        Ok(self.irnode)
    }


    fn finalize_let_slots(&mut self) -> Rerr {
        let base = self.local_alloc as usize;
        let mut let_count = 0usize;
        let infos: Vec<_> = self.symbols.values().filter_map(|entry| {
            if let SymbolEntry::Let(info_rc) = entry {
                Some(Rc::clone(info_rc))
            } else {
                None
            }
        }).collect();
        for info_rc in infos {
            let mut info = info_rc.borrow_mut();
            if info.needs_slot && info.slot.is_none() {
                let idx = base + let_count;
                if idx > u8::MAX as usize {
                    return errf!("let slot overflow");
                }
                let slot = idx as u8;
                self.reserve_slot(slot)?;
                info.slot = Some(slot);
                let_count += 1;
            }
        }
        self.local_alloc = (base + let_count) as u8;
        Ok(())
    }


}
