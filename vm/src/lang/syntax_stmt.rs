#[allow(dead_code)]
impl Syntax {
    pub fn item_param(&mut self) -> Ret<Box<dyn IRNode>> {
        let e = errf!("param format error");
        let end = self.tokens.len() - 1; // trailing synthetic sentinel
        if self.idx >= end {
            return e;
        }
        let mut nxt = self.next()?;
        if let Partition('{') = nxt {
        } else {
            return e;
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
        if params == 0 {
            return errf!("param must need at least one");
        }
        self.emit.source_map.register_param_names(param_names)?;
        Self::build_param_prelude(params as usize, false)
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
        use KwTy::*;
        let max = self.tokens.len() - 1;
        // let e0 = errf!("not find identifier '{}'", id);
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
            let nxt = self.tokens[self.idx].clone();
            if let Partition('(') = nxt {
                // function call
                return self.item_func_call(id);
            } else if let Keyword(sep @ (Dot | Colon | DColon)) = nxt {
                self.idx += 1;
                return self.parse_identifier_receiver_call(id, sep);
            }
        }
        self.link_symbol(&id)
    }

    pub fn item_may(&mut self) -> Ret<Option<Box<dyn IRNode>>> {
        use Bytecode::*;
        use KwTy::*;
        let max = self.tokens.len() - 1;
        if self.idx >= max {
            return Ok(None); // end
        }
        macro_rules! next {
            () => {{
                if self.idx >= max {
                    return errf!("item_may get next token error");
                }
                let nxt = &self.tokens[self.idx];
                self.idx += 1;
                nxt
            }};
        }
        let mut nxt = next!();
        let mut item: Box<dyn IRNode> = match nxt {
            Identifier(id) => self.item_identifier(id.clone())?,
            Keyword(This) => self.item_identifier("this".to_string())?,
            Keyword(Self_) => self.item_identifier("self".to_string())?,
            Keyword(Super) => self.item_identifier("super".to_string())?,
            Integer(n) => {
                let num_node = push_num(*n);
                if self.idx < self.tokens.len() {
                    if let Some((ty, inst)) = Self::parse_uint_suffix_cast(&self.tokens[self.idx]) {
                        Self::check_uint_literal_overflow(*n, ty)?;
                        self.idx += 1;
                        push_single(inst, num_node)
                    } else {
                        num_node
                    }
                } else {
                    num_node
                }
            }
            Token::Character(b) => push_num(*b as u128),
            Token::Address(a) => push_addr(*a),
            Token::Bytes(b) => push_bytes(b)?,
            Partition('(') => {
                let ckop = self.mode.check_op;
                self.mode.check_op = true;
                let exp = self.item_must(0)?;
                self.mode.check_op = ckop; // recover
                exp.checkretval()?; // must retv
                let e = errf!("(..) expression format error");
                nxt = next!();
                let Partition(')') = nxt else { return e };
                Box::new(IRNodeWrapOne { node: exp })
            }
            Partition('[') => {
                let mut subs = vec![];
                loop {
                    nxt = next!();
                    if let Partition(']') = nxt {
                        break;
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
                Box::new(self.item_may_block(self.mode.expect_retval)?)
            }
            Token::Operator(op) => match op {
                OpTy::NOT => {
                    let expr = self.item_must(0)?;
                    expr.checkretval()?; // must retv
                    push_single(NOT, expr)
                }
                _ => return errf!("operator {:?} cannot start expression", op),
            },
            Keyword(Not) => {
                let expr = self.item_must(0)?;
                expr.checkretval()?; // must retv
                push_single(NOT, expr)
            }
            Keyword(While) => {
                let exp = self.item_must(0)?;
                exp.checkretval()?; // must retv
                let suby = self.with_loop_scope(|s| s.item_may_list(false))?;
                push_double_box(IRWHILE, exp, suby)
            }
            Keyword(If) => {
                let exp = self.item_must(0)?;
                exp.checkretval()?; // must retv
                let keep_retval = self.mode.expect_retval;
                let list = self.item_may_list(keep_retval)?;
                let mut ifobj = IRNodeTriple {
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
                        return errf!("if expression must have else branch");
                    }
                    return Ok(Some(Box::new(ifobj)));
                };
                self.idx += 1; // over else token
                let nxt = &self.tokens[self.idx];
                // else
                let Keyword(If) = nxt else {
                    let elseobj = self.item_may_list(keep_retval)?;
                    ifobj.subz = elseobj;
                    return Ok(Some(Box::new(ifobj)));
                };
                // else if
                let elseifobj =
                    self.with_expect_retval(keep_retval, |s| match s.item_may()? {
                        Some(n) => Ok(n),
                        None => errf!("else if statement format error"),
                    })?;
                ifobj.subz = elseifobj;
                Box::new(ifobj)
            }
            Keyword(KwTy::Const) => {
                let e = errf!("const statement format error");
                let token = self.next()?;
                let Identifier(name) = token else { return e };
                let token = self.next()?;
                let Keyword(KwTy::Assign) = token else {
                    return e;
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
                    }
                    Token::Address(a) => a.to_readable(),
                    _ => unreachable!(),
                };
                if self.symbols.contains_key(&name) {
                    return errf!("symbol '{}' already defined", name);
                }
                self.symbols.insert(
                    name.clone(),
                    SymbolEntry::Const(dyn_clone::clone_box(val_node.as_ref())),
                );
                self.emit.source_map.register_const(name, val_str)?;
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
            }
            Keyword(Bind) => {
                let e = errf!("bind statement format error");
                let token = self.next()?;
                let Identifier(name) = token else { return e };
                let token = self.next()?;
                let Keyword(KwTy::Assign) = token else {
                    return e;
                };
                let expr = self.item_must(0)?;
                expr.checkretval()?; // must retv
                self.bind_macro(name.clone(), expr)?;
                return Ok(Some(push_empty()));
            }
            /* Keyword(Use) => { // use AnySwap = emqjNS9PscqdBpMtnC3Jfuc4mvZUPYTPS let e = errf!("use statement format error"); nxt = next!(); let Identifier(id) = nxt else { return e }; nxt = next!(); let Keyword(KwTy::Assign) = nxt else { return e }; nxt = next!(); let Token::Bytes(addr) = nxt else { return e }; self.bind_uses(id.clone(), addr.clone())?; push_empty() } */
            Keyword(Lib) => {
                if self.idx < max && matches!(self.tokens[self.idx], Partition('(')) {
                    self.parse_lib_receiver_call("lib(index) call format error")?
                } else {
                    let e = errf!("lib statement format error");
                    nxt = next!();
                    let Identifier(id) = nxt else { return e };
                    nxt = next!();
                    let Keyword(KwTy::Assign) = nxt else { return e };
                    nxt = next!();
                    let Integer(idx) = nxt else { return e };
                    let mut adr = None;
                    if self.idx < max && matches!(self.tokens[self.idx], Keyword(Colon)) {
                        self.idx += 1;
                        nxt = next!();
                        let Token::Address(a) = nxt else { return e };
                        adr = Some(*a as field::Address);
                    }
                    if *idx > u8::MAX as u128 {
                        return errf!("lib statement link index overflow");
                    }
                    self.bind_lib(id.clone(), *idx as u8, adr)?;
                    push_empty()
                }
            }
            Keyword(Param) => self.item_param()?,
            Keyword(Call) => {
                let first = self.next()?;
                let call = if let Ok(body) =
                    Self::parse_fixed_body_token::<CALL_BODY_WIDTH>(&first, "call body")
                {
                    decode_call_body(&body).map_err(|x| x.to_string())?
                } else {
                    let effect = Self::parse_call_effect_token(&first, "call effect format error")?;
                    let head = self.next()?;
                    let (target, fnsign) =
                        self.parse_generic_call_target_selector(head, "call target format error")?;
                    CallSpec::invoke(target, effect, fnsign)
                };
                let argv = self.deal_func_argv()?;
                push_user_invoke(call, argv)?
            }
            Keyword(UseCode) => {
                let first = self.next()?;
                let call = if let Ok(body) = Self::parse_usecode_body_token(&first) {
                    decode_usecode_body(&body).map_err(|x| x.to_string())?
                } else {
                    let (idx, fnsign) = self.parse_usecode_target_selector(first, "usecode target format error")?;
                    CallSpec::usecode(idx, fnsign)
                };
                push_user_splice(call)?
            }
            Keyword(CallExt) => self.parse_short_lib_call_invoke(
                CALLEXT,
                "callext body",
                "callext target format error",
                CallSpec::callext,
            )?,
            Keyword(CallView) => self.parse_short_lib_call_invoke(
                CALLVIEW,
                "callview body",
                "callview target format error",
                CallSpec::callview,
            )?,
            Keyword(CallPure) => self.parse_short_lib_call_invoke(
                CALLPURE,
                "callpure body",
                "callpure target format error",
                CallSpec::callpure,
            )?,
            Keyword(ByteCode) => {
                let e = errf!("bytecode format error");
                nxt = next!();
                let Partition('{') = nxt else { return e };
                let mut codes: Vec<u8> = Vec::new();
                loop {
                    let inst: u8;
                    match next!() {
                        Identifier(id) => {
                            let Some(t) = Bytecode::parse(id) else {
                                return errf!("bytecode {} not find", id);
                            };
                            inst = t as u8;
                        }
                        Integer(n) if *n <= u8::MAX as u128 => {
                            inst = *n as u8;
                        }
                        Partition('}') => break, // end
                        _ => return e,
                    }
                    codes.push(inst as u8);
                }
                Box::new(IRNodeBytecodes { codes })
            }

            Keyword(List) => {
                let e = errf!("list statement format error");
                nxt = next!();
                let Partition('{') = nxt else { return e };
                let mut subs = vec![];
                loop {
                    nxt = next!();
                    if let Partition('}') = nxt {
                        break;
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
                let Partition('{') = nxt else { return e };
                let mut subs = Vec::new();
                loop {
                    nxt = next!();
                    if let Partition('}') = nxt {
                        break;
                    } else {
                        self.idx -= 1;
                    }
                    let Some(k) = self.item_may()? else { break };
                    k.checkretval()?;
                    nxt = next!();
                    let Keyword(Colon) = nxt else { return e };
                    let Some(v) = self.item_may()? else { return e };
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
                // `log` consumes values from the stack (see interpreter: LOG1 pops 2, LOG2 pops 3, ...). Therefore log arguments must be parsed as value expressions.
                let max = self.tokens.len() - 1;
                if self.idx >= max {
                    return e;
                }
                let (open, close) = match &self.tokens[self.idx] {
                    Partition('(') => ('(', ')'),
                    Partition('{') => ('{', '}'),
                    Partition('[') => ('[', ']'),
                    _ => return e,
                };
                let mut subs =
                    Self::parse_delimited_value_exprs(self, open, close, "log argv number error")?;

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
            Keyword(Nil) => push_nil(),
            Keyword(True) => push_inst(PTRUE),
            Keyword(False) => push_inst(PFALSE),
            Keyword(Abort) => push_inst_noret(ABT),
            Keyword(End) => push_inst_noret(END),
            Keyword(Break) => {
                if self.mode.expect_retval {
                    return errf!("break statement cannot be used as expression");
                }
                if self.mode.loop_depth == 0 {
                    return errf!("break can only be used inside while loop");
                }
                push_inst_noret(IRBREAK)
            }
            Keyword(Continue) => {
                if self.mode.expect_retval {
                    return errf!("continue statement cannot be used as expression");
                }
                if self.mode.loop_depth == 0 {
                    return errf!("continue can only be used inside while loop");
                }
                push_inst_noret(IRCONTINUE)
            }
            Keyword(Print) => {
                let exp = self.item_must(0)?;
                if !exp.hasretval() {
                    return errf!("print arguments must be expressions with return values; do not use bind/var declarations directly");
                }
                push_single_noret(PRT, exp)
            }
            Keyword(Assert) => {
                let exp = self.item_must(0)?;
                if !exp.hasretval() {
                    return errf!("assert arguments must be expressions with return values");
                }
                push_single_noret(AST, exp)
            }
            Keyword(Throw) => {
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
}
