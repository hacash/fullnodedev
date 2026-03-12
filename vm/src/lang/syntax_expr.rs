#[allow(dead_code)]
impl Syntax {
    pub fn new(mut tokens: Vec<Token>) -> Self {
        use Bytecode::*;
        tokens.push(Token::Partition('}'));
        Self {
            tokens,
            emit: SyntaxEmit {
                irnode: IRNodeArray::with_opcode(IRBLOCK),
                ..Default::default()
            },
            mode: SyntaxMode {
                check_op: true,
                ..Default::default()
            },
            ..Default::default()
        }
    }

    fn assign_target_name(node: &dyn IRNode) -> Option<String> {
        if let Some(ir) = node.as_any().downcast_ref::<IRNodeParam1>() {
            return Some(ir.as_text().clone());
        }
        node.as_any()
            .downcast_ref::<IRNodeLeaf>()
            .map(|ir| ir.as_text().clone())
    }

    pub fn item_with_left(&mut self, mut left: Box<dyn IRNode>) -> Ret<Box<dyn IRNode>> {
        use Bytecode::*;
        use KwTy::*;
        let max = self.tokens.len();
        if self.idx >= self.tokens.len() {
            return Ok(left); // end
        }
        macro_rules! next {
            () => {{
                if self.idx >= max {
                    return errf!("item-with-left get next token failed");
                }
                let nxt = self.tokens[self.idx].clone();
                self.idx += 1;
                nxt
            }};
        }
        loop {
            if self.idx >= max {
                break;
            }
            let nxt = next!();
            match nxt {
                Partition('[') => {
                    // Allow indexing on any value expression, not just identifiers. This keeps decompile->recompile closed for `ITEMGET` nodes.
                    left.checkretval()?; // receiver must be a value expression
                    let k = self.item_must(0)?;
                    k.checkretval()?; // key must be a value expression
                    let Partition(']') = next!() else {
                        return errf!("item get statement format invalid");
                    };
                    left = Box::new(IRNodeDouble {
                        hrtv: true,
                        inst: ITEMGET,
                        subx: left,
                        suby: k,
                    });
                }
                Keyword(KwTy::Assign)
                | Keyword(AsgAdd)
                | Keyword(AsgSub)
                | Keyword(AsgMul)
                | Keyword(AsgDiv) => {
                    let e = errf!("assign statement format invalid");
                    let Some(id) = Self::assign_target_name(&*left) else { return e };
                    let v = self.item_must(0)?;
                    v.checkretval()?; // must retv
                    left = match nxt {
                        Keyword(KwTy::Assign) => self.save_local(&id, v)?,
                        _ => self.assign_local(&id, v, &nxt)?,
                    };
                }
                Keyword(As) => {
                    left.checkretval()?; // must retv
                    let e = errf!("<as> expression format invalid");
                    let nk = next!();
                    let Some(target_ty) = Self::parse_scalar_value_ty(&nk) else {
                        return e;
                    };
                    Self::check_literal_as_cast(&*left, target_ty)?;
                    let skip_cast = Self::literal_value_type(&*left)
                        .is_some_and(|lit| lit == target_ty);
                    if !skip_cast {
                        left = Self::build_cast_node(left, target_ty);
                    }
                }
                Keyword(Is) => {
                    let e = errf!("<is> expression format invalid");
                    let mut nk = next!();
                    let mut is_not = false;
                    if let Keyword(Not) = nk {
                        is_not = true;
                        nk = next!();
                    }
                    left.checkretval()?; // must retv
                    let subx = left;
                    let mut res = match &nk {
                        Keyword(List) => push_single(TLIST, subx),
                        Keyword(Map) => push_single(TMAP, subx),
                        Identifier(name) => {
                            let Ok(ty) = ValueTy::from_name(name) else {
                                return e
                            };
                            Self::build_is_node(subx, ty)
                        }
                        _ => {
                            let Some(ty) = Self::parse_scalar_value_ty(&nk)
                                .or_else(|| match nk {
                                    Keyword(Nil) => Some(ValueTy::Nil),
                                    _ => None,
                                })
                            else {
                                return e;
                            };
                            Self::build_is_node(subx, ty)
                        }
                    };
                    if is_not {
                        res = Box::new(IRNodeSingle {
                            hrtv: true,
                            inst: NOT,
                            subx: res,
                        })
                    }
                    left = res
                }
                Operator(op) if self.mode.check_op => {
                    if op == OpTy::NOT {
                        return errf!("operator ! cannot be binary");
                    }
                    self.idx -= 1;
                    self.mode.check_op = false;
                    let res = self.parse_next_op(left, 0)?;
                    self.mode.check_op = true;
                    left = res;
                }
                Keyword(And) | Keyword(Or) if self.mode.check_op => {
                    self.idx -= 1;
                    self.mode.check_op = false;
                    let res = self.parse_next_op(left, 0)?;
                    self.mode.check_op = true;
                    left = res;
                }
                _ => {
                    self.idx -= 1;
                    break;
                }
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
            left.checkretval()?; // must retv
            right.checkretval()?; // must retv
            left = Box::new(IRNodeDouble {
                hrtv: true,
                inst: op.bytecode(),
                subx: left,
                suby: right,
            });
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
            None => errf!("does not match next syntax node"),
        })
    }

    pub fn item_may_list(&mut self, keep_retval: bool) -> Ret<Box<dyn IRNode>> {
        // NOTE: do NOT unwrap single-item blocks here. We must preserve IRBLOCK/IRBLOCKR opcodes so ircode -> fitsh -> ircode can be byte-for-byte stable under any PrintOption settings.
        Ok(Box::new(self.item_may_block(keep_retval)?))
    }

    pub fn item_may_block(&mut self, keep_retval: bool) -> Ret<IRNodeArray> {
        // return type changed
        let inst = Self::opcode_irblock(keep_retval);
        let mut block = IRNodeArray::with_opcode(inst); // was IRNodeArray::with_opcode(inst);
        let end = self.tokens.len() - 1; // trailing synthetic sentinel
        if self.idx >= end {
            return errf!("block format invalid");
        }
        let nxt = &self.tokens[self.idx];
        let se = match nxt {
            Partition('{') => '}',
            Partition('(') => ')',
            Partition('[') => ']',
            _ => return errf!("block format invalid"),
        };
        self.idx += 1;
        let mut block_err = false;
        let mut closed = false;
        let mut terminated = false;
        self.with_expect_retval(keep_retval, |s| {
            loop {
                if s.idx >= end {
                    block_err = true;
                    break;
                }
                let nxt = &s.tokens[s.idx];
                if let Partition(sp) = nxt {
                    if *sp == se {
                        closed = true;
                        s.idx += 1;
                        break;
                    } else if matches!(sp, '}' | ')' | ']') {
                        block_err = true;
                        break;
                    }
                }
                if s.try_skip_redundant_terminal_end(terminated) {
                    continue;
                }
                if terminated {
                    return errf!("unreachable code after terminal statement")
                }
                let Some(li) = s.item_may()? else { break };
                terminated = Self::is_strong_terminator(&*li);
                block.push(li);
            }
            Ok::<(), Error>(())
        })?;
        if block_err || !closed {
            return errf!("block format invalid");
        }
        if keep_retval {
            match block.subs.last() {
                None => return errf!("block expression cannot be empty"),
                Some(last) if !last.hasretval() => {
                    return errf!("block expression must return a value")
                }
                _ => {}
            }
        }
        Ok(block)
    }
}
