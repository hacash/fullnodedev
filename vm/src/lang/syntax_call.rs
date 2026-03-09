#[allow(dead_code)]
impl Syntax {
    pub fn link_lib(&self, s: &String) -> Ret<u8> {
        match self.bdlibs.get(s).map(|d| d.0) {
            Some(i) => Ok(i),
            _ => errf!("cannot find any lib bind '{}'", s),
        }
    }

    pub fn bind_lib(&mut self, s: String, idx: u8, adr: Option<field::Address>) -> Rerr {
        if let Some(..) = self.bdlibs.get(&s) {
            return errf!("<use> cannot repeat bind the symbol '{}'", s);
        }
        if let Some(adr) = adr {
            adr.must_contract()?;
        }
        self.bdlibs.insert(s.clone(), (idx, adr.clone()));
        self.emit.source_map.register_lib(idx, s, adr)?;
        Ok(())
    }

    fn parse_lib_index_token(token: &Token) -> Ret<u8> {
        if let Integer(n) = token {
            if *n > u8::MAX as u128 {
                return errf!("call index overflow");
            }
            return Ok(*n as u8);
        }
        errf!("call index must be integer")
    }

    fn parse_fn_sig_str(s: &str) -> Ret<[u8; 4]> {
        let hex = s.strip_prefix("0x").unwrap_or(s);
        if hex.len() != 8 {
            return errf!("function signature must be 8 hex digits, got '{}'", s);
        }
        let bytes = match hex::decode(hex) {
            Ok(b) => b,
            Err(_) => return errf!("function signature '{}' decode error", s),
        };
        let arr: [u8; 4] = match bytes.as_slice().try_into() {
            Ok(a) => a,
            Err(_) => return errf!("function signature expects 4 bytes"),
        };
        Ok(arr)
    }

    fn parse_fn_sig_token(token: &Token) -> Ret<[u8; 4]> {
        match token {
            Identifier(name) => Self::parse_fn_sig_str(name),
            Bytes(bytes) if bytes.len() == 4 => {
                let arr: [u8; 4] = bytes.as_slice().try_into().unwrap();
                Ok(arr)
            }
            Integer(n) if *n <= u32::MAX as u128 => {
                let v = *n as u32;
                Ok(v.to_be_bytes())
            }
            Bytes(..) => errf!("function signature bytes must be exactly 4 bytes"),
            _ => {
                errf!("function signature must be hex identifier, decimal <u32>, or 4-byte literal")
            }
        }
    }

    fn parse_named_func_sig_token(token: &Token) -> Ret<[u8; 4]> {
        match token {
            Identifier(func) => Self::parse_fn_sig_str(func).or_else(|_| Ok(calc_func_sign(func))),
            _ => Self::parse_fn_sig_token(token),
        }
    }

    fn parse_call_selector_token(&mut self, token: &Token) -> Ret<[u8; 4]> {
        let sig = Self::parse_named_func_sig_token(token)?;
        if let Identifier(func) = token {
            if Self::parse_fn_sig_str(func).is_err() {
                self.emit.source_map.register_func(sig, func.clone())?;
            }
        }
        Ok(sig)
    }

    fn parse_fixed_body_token<const N: usize>(token: &Token, label: &str) -> Ret<[u8; N]> {
        match token {
            Bytes(bytes) if bytes.len() == N => bytes.as_slice().try_into().map_err(|_| format!("{} expects fixed width", label)),
            Identifier(hex) => {
                let raw = hex.strip_prefix("0x").unwrap_or(hex.as_str());
                if raw.len() != N * 2 {
                    return errf!("{} expects {} hex digits", label, N * 2)
                }
                let bytes = hex::decode(raw).map_err(|_| format!("{} decode error", label))?;
                bytes.as_slice().try_into().map_err(|_| format!("{} expects fixed width", label).into())
            }
            Bytes(..) => errf!("{} expects {} bytes", label, N),
            _ => errf!("{} must be {}-byte literal", label, N),
        }
    }



    fn parse_usecode_body_token(token: &Token) -> Ret<[u8; USECODE_BODY_WIDTH]> {
        Self::parse_fixed_body_token(token, "usecode body")
    }


    fn expect_partition(&mut self, ch: char, err_msg: &'static str) -> Ret<()> {
        let token = self.next()?;
        let Partition(got) = token else {
            return errf!("{}", err_msg)
        };
        if got != ch {
            return errf!("{}", err_msg)
        }
        Ok(())
    }

    fn expect_keyword_token(&mut self, kw: KwTy, err_msg: &'static str) -> Ret<()> {
        let token = self.next()?;
        let Keyword(got) = token else {
            return errf!("{}", err_msg)
        };
        if got != kw {
            return errf!("{}", err_msg)
        }
        Ok(())
    }



    fn parse_lib_ctor_index(&mut self, err_msg: &'static str) -> Ret<u8> {
        self.expect_partition('(', err_msg)?;
        let token = self.next()?;
        let idx = Self::parse_lib_index_token(&token)?;
        self.expect_partition(')', err_msg)?;
        Ok(idx)
    }

    fn parse_call_effect_token(token: &Token, err_msg: &'static str) -> Ret<EffectMode> {
        Ok(match token {
            Keyword(KwTy::Edit) => EffectMode::Edit,
            Keyword(KwTy::View) => EffectMode::View,
            Keyword(KwTy::Pure) => EffectMode::Pure,
            _ => return errf!("{}", err_msg),
        })
    }

    fn parse_call_target_head(&mut self, head: Token, err_msg: &'static str) -> Ret<CallTarget> {
        Ok(match head {
            Keyword(KwTy::This) => CallTarget::This,
            Keyword(KwTy::Self_) => CallTarget::Self_,
            Keyword(KwTy::Upper) => CallTarget::Upper,
            Keyword(KwTy::Super) => CallTarget::Super,
            Keyword(KwTy::Lib) => CallTarget::Call(self.parse_lib_ctor_index(err_msg)?),
            Keyword(KwTy::Use) => CallTarget::Use(self.parse_lib_ctor_index(err_msg)?),
            Identifier(id) => CallTarget::Call(self.link_lib(&id)?),
            Integer(..) => CallTarget::Call(Self::parse_lib_index_token(&head)?),
            _ => return errf!("{}", err_msg),
        })
    }

    fn try_skip_redundant_terminal_end(&mut self, terminated: bool) -> bool {
        if !terminated {
            return false;
        }
        if self.idx < self.tokens.len() && matches!(self.tokens[self.idx], Keyword(KwTy::End)) {
            self.idx += 1;
            return true;
        }
        false
    }

    fn parse_generic_call_target_selector(
        &mut self,
        head: Token,
        err_msg: &'static str,
    ) -> Ret<(CallTarget, [u8; 4])> {
        let target = self.parse_call_target_head(head, err_msg)?;
        self.expect_keyword_token(KwTy::Dot, err_msg)?;
        let selector = self.next()?;
        Ok((target, self.parse_call_selector_token(&selector)?))
    }

    fn is_strong_terminator(node: &dyn IRNode) -> bool {
        use Bytecode::*;
        if let Some(wrap) = node.as_any().downcast_ref::<IRNodeWrapOne>() {
            return Self::is_strong_terminator(&*wrap.node);
        }
        let op: Bytecode = std_mem_transmute!(node.bytecode());
        match op {
            RET | END | ERR | ABT | USECODE => true,
            IRBLOCK | IRBLOCKR | IRLIST => node
                .as_any()
                .downcast_ref::<IRNodeArray>()
                .and_then(|arr| arr.subs.last())
                .is_some_and(|last| Self::is_strong_terminator(&**last)),
            IRIF | IRIFR => node
                .as_any()
                .downcast_ref::<IRNodeTriple>()
                .is_some_and(|ifn| {
                    Self::is_strong_terminator(&*ifn.suby)
                        && Self::is_strong_terminator(&*ifn.subz)
                }),
            _ => false,
        }
    }




    fn parse_usecode_target_selector(&mut self, head: Token, err_msg: &'static str) -> Ret<(u8, [u8; 4])> {
        let idx = match head {
            Identifier(id) => self.link_lib(&id)?,
            Integer(..) => Self::parse_lib_index_token(&head)?,
            Keyword(KwTy::Lib) => self.parse_lib_ctor_index(err_msg)?,
            _ => return errf!("{}", err_msg),
        };
        let sep = self.next()?;
        let Keyword(sep) = sep else {
            return errf!("{}", err_msg)
        };
        if !matches!(sep, KwTy::Dot | KwTy::DColon) {
            return errf!("{}", err_msg)
        }
        let selector = self.next()?;
        Ok((idx, self.parse_call_selector_token(&selector)?))
    }

    fn parse_shortcut_lib_selector(&mut self, head: Token, err_msg: &'static str) -> Ret<(u8, [u8; 4])> {
        let idx = match head {
            Identifier(id) => self.link_lib(&id)?,
            Integer(..) => Self::parse_lib_index_token(&head)?,
            Keyword(KwTy::Lib) => self.parse_lib_ctor_index(err_msg)?,
            _ => return errf!("{}", err_msg),
        };
        self.expect_keyword_token(KwTy::DColon, err_msg)?;
        let selector = self.next()?;
        Ok((idx, self.parse_call_selector_token(&selector)?))
    }




    fn parse_lib_receiver_call(&mut self, err_msg: &'static str) -> Ret<Box<dyn IRNode>> {
        let idx = self.parse_lib_ctor_index(err_msg)?;
        let effect = match self.next()? {
            Keyword(KwTy::Dot) => EffectMode::Edit,
            Keyword(KwTy::Colon) => EffectMode::View,
            Keyword(KwTy::DColon) => EffectMode::Pure,
            _ => return errf!("{}", err_msg),
        };
        let selector = self.next()?;
        let sig = self.parse_call_selector_token(&selector)?;
        let argv = self.deal_func_argv()?;
        push_user_invoke(CallSpec::invoke(CallTarget::Call(idx), effect, sig), argv)
    }

    fn parse_short_lib_call_spec<F>(
        &mut self,
        first: Token,
        inst: Bytecode,
        body_label: &'static str,
        err_msg: &'static str,
        build: F,
    ) -> Ret<CallSpec>
    where
        F: FnOnce(u8, FnSign) -> CallSpec,
    {
        if let Ok(body) = Self::parse_fixed_body_token::<5>(&first, body_label) {
            return decode_user_call_site(inst, &body).map_err(|x| x.to_string());
        }
        let (idx, fnsign) = self.parse_shortcut_lib_selector(first, err_msg)?;
        Ok(build(idx, fnsign))
    }

    fn parse_short_lib_call_invoke<F>(
        &mut self,
        inst: Bytecode,
        body_label: &'static str,
        err_msg: &'static str,
        build: F,
    ) -> Ret<Box<dyn IRNode>>
    where
        F: FnOnce(u8, FnSign) -> CallSpec,
    {
        let first = self.next()?;
        let call = self.parse_short_lib_call_spec(first, inst, body_label, err_msg, build)?;
        let argv = self.deal_func_argv()?;
        push_user_invoke(call, argv)
    }

    fn parse_identifier_receiver_call(&mut self, id: String, sep: KwTy) -> Ret<Box<dyn IRNode>> {
        let selector = self.next()?;
        let fnsign = self.parse_call_selector_token(&selector)?;
        let argv = self.deal_func_argv()?;
        let call = match sep {
            KwTy::Dot => match id.as_str() {
                "this" => CallSpec::invoke(CallTarget::This, EffectMode::Edit, fnsign),
                "self" => CallSpec::invoke(CallTarget::Self_, EffectMode::Edit, fnsign),
                "super" => CallSpec::invoke(CallTarget::Super, EffectMode::Edit, fnsign),
                _ => CallSpec::callext(self.link_lib(&id)?, fnsign),
            },
            KwTy::Colon | KwTy::DColon => {
                let effect = maybe!(sep == KwTy::DColon, EffectMode::Pure, EffectMode::View);
                match id.as_str() {
                    "self" => CallSpec::invoke(CallTarget::Self_, effect, fnsign),
                    "this" | "super" => return errf!("call express after identifier format error"),
                    _ => CallSpec::invoke(CallTarget::Call(self.link_lib(&id)?), effect, fnsign),
                }
            }
            _ => never!(),
        };
        push_user_invoke(call, argv)
    }
}
