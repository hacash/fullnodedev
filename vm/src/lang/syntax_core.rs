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
            let prev_check_op = s.mode.check_op;
            // Argument lists rely on expression boundaries because the tokenizer
            // drops commas. Nested arg expressions must therefore always parse
            // operators normally, even when the caller is currently parsing the
            // RHS of an outer binary expression.
            s.mode.check_op = true;
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
            s.mode.check_op = prev_check_op;
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

    /* pub fn bind_uses(&mut self, s: String, adr: Vec<u8>) -> Rerr { if let Some(..) = self.bduses.get(&s) { return errf!("<use> cannot repeat bind the symbol '{}'", s) } let addr = Address::from_vec(adr); addr.must_contract()?; self.bduses.insert(s, addr); Ok(()) } pub fn link_use(&self, s: &String) -> Ret<Vec<u8>> { match self.bduses.get(s) { Some(i) => Ok(i.to_vec()), _ =>  errf!("cannot find any use bind '{}'", s) } } */

    fn next(&mut self) -> Ret<Token> {
        if self.idx >= self.tokens.len() {
            return errf!("item_with_left: get next token failed");
        }
        let nxt = &self.tokens[self.idx];
        self.idx += 1;
        Ok(nxt.clone())
    }

    fn with_expect_retval<F, R>(&mut self, expect: bool, f: F) -> R
    where
        F: FnOnce(&mut Self) -> R,
    {
        let prev = self.mode.expect_retval;
        self.mode.expect_retval = expect;
        let res = f(self);
        self.mode.expect_retval = prev;
        res
    }

    fn with_loop_scope<F, R>(&mut self, f: F) -> R
    where
        F: FnOnce(&mut Self) -> R,
    {
        self.mode.loop_depth += 1;
        let res = f(self);
        self.mode.loop_depth -= 1;
        res
    }

    fn build_param_prelude(params: usize, allow_zero: bool) -> Ret<Box<dyn IRNode>> {
        use Bytecode::*;
        match params {
            0 if allow_zero => Ok(push_inst_noret(POP)),
            1 => Ok(push_single_p1(PUT, 0, push_inst(ROLL0))),
            // `param { ... }` is just compiler sugar for unpacking the current value;
            // it intentionally does not enforce Tuple-only ABI rules.
            2.. => Ok(push_double(UNPACK, ROLL0, P0)),
            _ => errf!("at least one param required"),
        }
    }
}
