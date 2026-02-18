

impl Syntax {

    fn parse_paren_argv_items(&mut self) -> Ret<Vec<Box<dyn IRNode>>> {
        // Parse `(...)` argument lists as a sequence of value expressions. Note: the tokenizer ignores commas, so argument separation is by expression boundaries.
        self.parse_delimited_value_exprs('(', ')', "call argv format error")
    }

    pub fn item_get(&mut self, id: String) -> Ret<Box<dyn IRNode>> {
        use Bytecode::*;
        let Some(_) = self.symbols.get(&id).and_then(|entry| match entry {
            SymbolEntry::Slot(_, _) => Some(()),
            _ => None,
        }) else {
            return errf!("cannot find '{}' object in item get", id)
        };
        let k = self.item_must(1)?;  // over [
        k.checkretval()?; // ITEMGET consumes a key value from the stack
        let Partition(']') = self.next()? else {
            return errf!("item get statement format error")
        };
        let obj = self.link_local(&id)?;
        let nd = IRNodeDouble{hrtv: true, inst: ITEMGET, subx: obj, suby: k};
        Ok(Box::new(nd))
    }


    pub fn must_get_func_argv(&mut self, md: ArgvMode) -> Ret<(usize, Box<dyn IRNode>)> {
        let argvs = self.parse_paren_argv_items()?;
        let alen = argvs.len();
        let argv = match md {
            ArgvMode::Concat => concat_func_argvs(argvs)?,
            ArgvMode::List => pack_func_argvs(argvs)?,
        };
        Ok((alen, argv))

    }

    pub fn item_func_call(&mut self, id: String) -> Ret<Box<dyn IRNode>> {
        // ir func
        if let Some((_, inst, pms, args, rs)) = pick_ir_func(&id) {
            let argvs = self.parse_paren_argv_items()?;
            for arg in &argvs {
                arg.checkretval()?;
            }
            if pms + args != argvs.len() {
                return errf!("ir func call argv length must {} but got {}", 
                    pms + args, argvs.len()
                )
            }
            if rs > 1 {
                return errf!(
                    "ir func '{}' has unsupported multi-value return ({})",
                    id, rs
                )
            }
            return build_ir_func(inst, pms, args, rs, argvs,)
        }

        // native func (pure, concat args; arity checked by name)
        if let Some(idx) = pick_native_func(&id) {
            let (num, argvs) = self.must_get_func_argv(ArgvMode::Concat)?;
            let Some(need) = NativeFunc::argv_len(idx) else {
                return errf!("unknown native func idx {}", idx)
            };
            if num != need {
                return errf!(
                    "native func '{}' requires {} argument(s) but got {}",
                    id,
                    need,
                    num
                )
            }
            return Ok(push_single_p1_hr(true, Bytecode::NTFUNC, idx, argvs));
        }

        // native env (VM context read, 0 args)
        if let Some(idx) = pick_native_env(&id) {
            let (num, argvs) = self.must_get_func_argv(ArgvMode::Concat)?;
            let allow_empty_placeholder = num == 1
                && argvs
                    .as_any()
                    .downcast_ref::<IRNodeLeaf>()
                    .is_some_and(|leaf| leaf.inst == Bytecode::PNBUF);
            if num != 0 && !allow_empty_placeholder {
                return errf!("native env '{}' takes no arguments but got {}", id, num)
            }
            return Ok(push_single_p1_hr(true, Bytecode::NTENV, idx, push_empty()));
        }

        // extend action
        if let Some((hrtv, inst, para, args_len)) = pick_ext_func(&id) {
            let (num, argvres) = self.must_get_func_argv(ArgvMode::Concat)?;
            let allow_empty_placeholder = args_len == 0
                && num == 1
                && argvres
                    .as_any()
                    .downcast_ref::<IRNodeLeaf>()
                    .is_some_and(|leaf| leaf.inst == Bytecode::PNBUF);
            if num != args_len && !allow_empty_placeholder {
                 return errf!("extend function/action '{}' argv length must {} but got {}", 
                    id, args_len, num
                )
            }
            let subx = if inst.metadata().input == 0 { push_empty() } else { argvres };
            return Ok(push_single_p1_hr(hrtv, inst, para, subx));
        }

        // not find
        return errf!("unknown function '{}'", id);
    }

}



fn build_ir_func(inst: Bytecode, pms: usize, args: usize, rs: usize, argvs: Vec<Box<dyn IRNode>>) -> Ret<Box<dyn IRNode>> {
    use Bytecode::*;
    let mut argvs = std::collections::VecDeque::from(argvs);
    let hrtv = maybe!(rs==1, true, false);
    let ttv = pms + args;
    if ttv == 0 {
        return Ok(Box::new(IRNodeLeaf::notext(hrtv, inst)))
    }
    macro_rules! avg {() => {
        argvs.pop_front().unwrap()       
    }}
    macro_rules! param { () => {{
        let mut para = -1i16;
        let e = errf!("ir func call param error");
        let ag = avg!();
        if let Some(n) = ag.as_any().downcast_ref::<IRNodeParam1>() {
            para = n.para as i16;
        } else if let Some(n) = ag.as_any().downcast_ref::<IRNodeParam2>() {
            para = i16::from_be_bytes(n.para);
        } else if let Some(n) = ag.as_any().downcast_ref::<IRNodeLeaf>() {
            para = match n.inst {
                P0 | GET0 => 0,
                P1 | GET1 => 1,
                P2 | GET2 => 2,
                P3 | GET3 => 3,
                _ => return e
            }
        }
        if para == -1 || para > 255 {
            return e
        }
        para as u8
    }}}
    if pms == 0 {
        return Ok(match args {
            1 => Box::new(IRNodeSingle{hrtv, inst, subx: avg!()}),
            2 => Box::new(IRNodeDouble{hrtv, inst, subx: avg!(), suby: avg!()}),
            3 => {
                // Special-case CHOOSE: source syntax is choose(cond, yes, no) IRNodeTriple expects (subx, suby, subz) which codegen will emit in order and the runtime expects stack [subx, suby, subz]. To match natural `choose(cond, yes, no)` call order we rearrange arguments so that runtime selection logic works: produce IRNodeTriple{subx=yes, suby=no, subz=cond}.
                if inst == Bytecode::CHOOSE {
                    let a = avg!(); // cond
                    let b = avg!(); // yes
                    let c = avg!(); // no
                    // To make `choose(cond, yes, no)` select `yes` when cond is true (interpreter: pop cond; if false swap; pop unchosen), arrange IR children so that codegen emits [yes, no, cond] -> subx = yes, suby = no, subz = cond.
                    Box::new(IRNodeTriple{hrtv, inst, subx: b, suby: c, subz: a})
                } else {
                    Box::new(IRNodeTriple{hrtv, inst, subx: avg!(), suby: avg!(), subz: avg!()})
                }
            }
            _ => unreachable!()
        })
    }
    if pms == 1 {
        let para = param!();
        return Ok(match args {
            0 => Box::new(IRNodeParam1{hrtv, inst, para, text:s!("")}),
            1 => push_single_p1_hr(hrtv, inst, para, avg!()),
            _ => unreachable!()
        })
    }
    if pms == 2 {
        let p1 = param!();
        let p2 = param!();
        return Ok(match args {
            0 => Box::new(IRNodeParam2{hrtv, inst, para: [p1, p2]}),
            1 => Box::new(IRNodeParam2Single{hrtv, inst, para: [p1, p2], subx: avg!()}),
            _ => unreachable!()
        })
    }

    errf!("cannot match ir call type: params({}), args({})", pms, args)
}




/****************************** */



fn pick_ir_func(id: &str) -> Option<(IrFn, Bytecode, usize, usize, usize)> {
    IrFn::from_name(id)
}


fn pick_native_func(id: &str) -> Option<u8> {
    NativeFunc::from_name(id).map(|d| d.0)
}

fn pick_native_env(id: &str) -> Option<u8> {
    NativeEnv::from_name(id).map(|d| d.0)
}


fn concat_func_argvs(mut list: Vec<Box<dyn IRNode>>) -> Ret<Box<dyn IRNode>> {
    // list.reverse();
    use Bytecode::*;
    let Some(mut res) = list.pop() else {
        return Ok(push_inst(PNBUF))
    };
    while let Some(x) = list.pop() {
        res = Box::new(IRNodeDouble{hrtv:true, inst:CAT, subx: x, suby: res});
    }
    Ok(res)
}


fn pack_func_argvs(mut subs: Vec<Box<dyn IRNode>>) -> Ret<Box<dyn IRNode>> {
    use Bytecode::*;
    // list.reverse();
    let argv_len = subs.len();
    Ok(match argv_len {
        0 => push_nil(),
        1 => subs.pop().unwrap(),
        2..=15 => {
            let num = push_num(argv_len as u128);
            let pklist = push_inst(PACKLIST);
            subs.push(num);
            subs.push(pklist);
            Box::new(Syntax::build_irlist(subs)?)
        },
        _ => return errf!("function argv length cannot more than 15"),
    })
    /* let mut res = list.pop().unwrap(); while let Some(x) = list.pop() { res = Box::new(IRNodeDouble{hrtv:true, inst:Bytecode::CAT, subx: x, suby: res}); } res */
}



/* return (hav_revt, code, para, args_len) */
fn pick_ext_func(id: &str) -> Option<(bool, Bytecode, u8, usize)> {
    if let Some(x) = CALL_EXTEND_ENV_DEFS.iter().find(|f|f.1==id) {
        return Some((true, Bytecode::EXTENV,  x.0, x.3))
    }
    if let Some(x) = CALL_EXTEND_VIEW_DEFS.iter().find(|f|f.1==id) {
        return Some((true, Bytecode::EXTVIEW, x.0, x.3))
    }
    if let Some(x) = CALL_EXTEND_ACTION_DEFS.iter().find(|f|f.1==id) {
        return Some((false, Bytecode::EXTACTION, x.0, x.3))
    }
    None
}
