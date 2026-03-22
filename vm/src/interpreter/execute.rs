/**
* parse bytecode params
*/
use crate::machine::{DeferredRegistry, VmHost};

#[inline(always)]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe fn read_arr<const N: usize>(codes: &[u8], pc: usize) -> [u8; N] {
    let mut out = [0u8; N];
    std::ptr::copy_nonoverlapping(codes.as_ptr().add(pc), out.as_mut_ptr(), N);
    out
}

#[inline(always)]
fn finish_ntcall(
    cap: &SpaceCap,
    gst: &GasExtra,
    step_gas_use: &mut GasUse,
    ops: &mut Stack,
    r: Value,
    g: i64,
) -> VmrtRes<()> {
    let r = r.valid(cap)?;
    step_gas_use.resource += gst.nt_bytes(r.val_size());
    step_gas_use.resource += g;
    ops.push(r)?;
    Ok(())
}

macro_rules! itrparam {
    ($codes: expr, $pc: expr, $l: expr, $t: ty) => {{
        let r = $pc + $l;
        #[cfg(debug_assertions)]
        if r < $pc || r > $codes.len() {
            return itr_err_code!(CodeOverflow);
        }
        let v = <$t>::from_be_bytes(unsafe { read_arr::<$l>($codes, $pc) });
        $pc = r;
        v
    }};
}

macro_rules! itrparamu8 {
    ($codes: expr, $pc: expr) => {
        itrparam! {$codes, $pc, 1, u8}
    };
}

macro_rules! itrparamu16 {
    ($codes: expr, $pc: expr) => {
        itrparam! {$codes, $pc, 2, u16}
    };
}

macro_rules! peekparam {
    ($codes: expr, $pc: expr, $l: expr, $t: ty) => {{
        let _r = $pc + $l;
        #[cfg(debug_assertions)]
        if _r < $pc || _r > $codes.len() {
            return itr_err_code!(CodeOverflow);
        }
        <$t>::from_be_bytes(unsafe { read_arr::<$l>($codes, $pc) })
    }};
}

macro_rules! peekparamu8 {
    ($codes: expr, $pc: expr) => {
        peekparam! {$codes, $pc, 1, u8}
    };
}

macro_rules! peekparamu16 {
    ($codes: expr, $pc: expr) => {
        peekparam! {$codes, $pc, 2, u16}
    };
}

macro_rules! itrparambufex {
    ($codes: expr, $pc: expr, $l: expr, $t: ty) => {{
        let s = itrparam! {$codes, $pc, $l, $t} as usize;
        let l = $pc;
        let r = l + s;
        if r < l || r > $codes.len() {
            return itr_err_code!(CodeOverflow);
        }
        $pc = r;
        let v = unsafe { std::slice::from_raw_parts($codes.as_ptr().add(l), s) };
        Value::Bytes(v.to_vec())
    }};
}

macro_rules! itrparambuf {
    ($codes: expr, $pc: expr) => {
        itrparambufex!($codes, $pc, 1, u8)
    };
}

macro_rules! itrparambufl {
    ($codes: expr, $pc: expr) => {
        itrparambufex!($codes, $pc, 2, u16)
    };
}

macro_rules! jump {
    ($codes: expr, $pc: expr, $l: expr) => {{
        let tpc = match $l {
            1 => peekparamu8!($codes, $pc) as usize,
            2 => peekparamu16!($codes, $pc) as usize,
            _ => return itr_err_code!(CodeOverflow),
        };
        $pc = tpc; // jump to
    }};
}

macro_rules! ostjump {
    ($codes: expr, $pc: expr, $l: expr) => {{
        let tpc = match $l {
            1 => itrparam! {$codes, $pc, 1, i8} as isize,
            2 => itrparam! {$codes, $pc, 2, i16} as isize,
            _ => return itr_err_code!(CodeOverflow),
        };
        let tpc = ($pc as isize + tpc);
        if tpc < 0 {
            return itr_err_code!(CodeOverflow);
        }
        $pc = tpc as usize; // jump to
    }};
}

macro_rules! branch {
    ( $ops: expr, $codes: expr, $pc: expr, $l: expr) => {
        if $ops.pop()?.extract_bool()? {
            jump!($codes, $pc, $l);
        } else {
            $pc += $l;
        }
    };
}

macro_rules! ostbranchex {
    ( $ops: expr, $codes: expr, $pc: expr, $l: expr, $expect: expr) => {
        if $ops.pop()?.extract_bool()? == $expect {
            ostjump!($codes, $pc, $l);
        } else {
            $pc += $l;
        }
    };
}
// is_not_zero
macro_rules! ostbranch {
    ( $ops: expr, $codes: expr, $pc: expr, $l: expr) => {
        ostbranchex!($ops, $codes, $pc, $l, true)
    };
}

/*
* Execution hot path intentionally trusts verified bytecode.
* BUG1/2/3 are not runtime bugs: param reads and jumps omit repeated checks,
* and gas negativity is finalized after each instruction for throughput.
* Callers must only execute bytecode already accepted by rt/verify.
* Callers must also seed operand/local stacks with values already valid under SpaceCap.
*/

pub fn execute_code<H: VmHost + ?Sized>(
    // frame local
    pc: &mut usize,
    codes: &[u8],
    exec: ExecCtx,
    operands: &mut Stack,
    locals: &mut Stack,
    heap: &mut Heap,
    context_addr: &field::Address,
    current_addr: &field::Address,
    // shared runtime
    gas_table: &GasTable,
    gas_extra: &GasExtra,
    space_cap: &SpaceCap,
    gas_use: &mut GasUse,
    global_map: &mut GKVMap,
    memory_map: &mut CtcKVMap,
    deferred_registry: &mut DeferredRegistry,
    host: &mut H,
) -> VmrtRes<CallExit> {

    use Bytecode::*;
    use CallExit::*;
    use ItrErrCode::*;
    use Value::*;

    let cap = space_cap;
    let ops = operands;
    let gst = gas_extra;
    let hei: u64 = host.height();

    // check code length
    // let codelen = codes.len();
    // let tail = codelen;

    macro_rules! nsr { () => { if exec.effect == EffectMode::Pure { return itr_err_code!(InstDisabled); } }; } // not read in pure mode
    macro_rules! nsw { () => { if matches!(exec.effect, EffectMode::Pure | EffectMode::View) { return itr_err_code!(InstDisabled); } }; } // not write in view/pure mode
    macro_rules! pu8 { () => { itrparamu8!(codes, *pc) }; }
    macro_rules! pty { () => { ops.peek()?.ty() }; }
    macro_rules! ptyn { () => { ops.peek()?.ty_num() }; }
    macro_rules! pu8_as_u16 { () => { pu8!() as u16 }; }
    macro_rules! pu16 { () => { itrparamu16!(codes, *pc) }; }
    macro_rules! pbuf { () => { itrparambuf!(codes, *pc) }; }
    macro_rules! pbufl { () => { itrparambufl!(codes, *pc) }; }
    macro_rules! ops_pop_to_u16 { () => { ops.pop()?.extract_u16()? }; }
    macro_rules! ops_peek_to_u16 { () => { ops.peek()?.extract_u16()? }; }
    macro_rules! check_compo_type { ($m: ident) => { match ops.compo() { Ok(c) => c.$m(), _ => false, } }; }
    
    enum Step {
        Continue,
        Exit(CallExit),
    }

    // start run
    loop {
        // read inst
        debug_assert!(*pc < codes.len());
        // if *pc >= codes.len() {
        //     return itr_err_code!(CodeOverflow)
        // }
        let instbyte = unsafe { *codes.get_unchecked(*pc as usize) }; // u8
        let instruction: Bytecode = std_mem_transmute!(instbyte);
        *pc += 1; // next

        // debug_print_stack(ops, locals, pc, instruction);
        
        
        // gas
        let base_gas = gas_table.gas(instbyte);
        let mut step_gas_use = GasUse {
            compute: base_gas,
            ..GasUse::default()
        };

        macro_rules! gas_add {
            ($kind:ident, raw, $n:expr) => {{
                step_gas_use.$kind += ($n) as i64;
            }};
            ($kind:ident, $f:ident $(, $arg:expr)* $(,)?) => {{
                step_gas_use.$kind += gst.$f($($arg),*);
            }};
        }
        macro_rules! gas_resource {
            ($f:ident $(, $arg:expr)* $(,)?) => {{
                gas_add!(resource, $f $(, $arg)*);
            }};
        }

        macro_rules! actcall { ($act_kind: expr) => {{
            let act_kind = $act_kind;
            let idx = pu8!();
            let pass_body = act_pass_body(act_kind);
            let have_retv = act_have_retv(act_kind);
            ensure_act_allowed(exec, act_kind, idx)?;
            let kid = u16::from_be_bytes([instbyte, idx]);
            let mut actbody = vec![];
            if pass_body {
                let mut bdv = ops.peek()?.extract_call_data(heap)?;
                actbody.append(&mut bdv);
                gas_resource!(act_bytes, actbody.len());
            }
            let (bgasu, cres) = host.action_call(kid, actbody).map_err(|e|
                ItrErr::new(maybe!(e.is_revert(), ActCallRevert, ActCallError), e.as_str()))?;
            gas_add!(resource, raw, bgasu);
            if have_retv {
                let resv = Value::type_from(act_retv_type(act_kind, idx)?, cres)?.valid(cap)?;
                gas_resource!(act_bytes, resv.val_size());
                if pass_body {
                    *ops.peek()? = resv;
                } else {
                    ops.push(resv)?;
                }
            } else {
                ops.pop()?;
            }
        }}}

        macro_rules! local_get {
            ($idx: expr) => {{
                let v = locals.load($idx as usize)?.valid(cap)?;
                gas_resource!(stack_copy, v.dup_size());
                ops.push(v)?;
            }};
        }

        macro_rules! wlog {
            ($itn: expr) => {{
                nsw!();
                let items = ops.popn($itn)?;
                gas_add!(storage, log_bytes, items.iter().map(|v| v.val_size()).sum());
                host.log_push(context_addr, items)?;
            }};
        }

        macro_rules! peek_op_gas { ($method:ident($($arg:expr),*)) => {{
            let (v, outlen) = ops.peek_with_size()?;
            gas_resource!(stack_op, outlen);
            v.$method($($arg),*)?;
        }}}

        macro_rules! compare_gas {
            () => {{
                if ops.datas.len() >= 2 {
                    let l = ops.datas.len();
                    gas_resource!(stack_cmp, lgc_compare_fee(&ops.datas[l - 2], &ops.datas[l - 1]));
                }
            }};
        }

        macro_rules! push_buf_gas {
            ($v:expr) => {{
                let v = $v.valid(cap)?;
                if let Some(b) = v.match_bytes() {
                    gas_resource!(stack_copy, b.len());
                }
                ops.push(v)?;
            }};
        }

        macro_rules! hwrite {
            ($idx:expr) => {{
                let v = ops.pop()?;
                gas_resource!(heap_write, v.val_size());
                heap.write($idx, v)?;
            }};
        }

        macro_rules! hread_push {
            ($v:expr) => {{
                let v = $v;
                gas_resource!(heap_read, v.val_size());
                ops.push(v)?;
            }};
        }

        macro_rules! kvput_inner {
            ($store:expr, $key_cost:expr) => {{
                let v = ops.pop()?.valid(cap)?;
                let vlen = v.val_size();
                let k = ops.pop()?;
                let klen = k.extract_key_bytes()?.len();
                let is_new = !$store.contains_key(&k)?;
                gas_resource!(stack_write, klen);
                gas_resource!(stack_write, vlen);
                if is_new {
                    gas_add!(resource, raw, $key_cost);
                }
                $store.put(k, v)?;
            }};
        }

        macro_rules! kvput {
            ($store:expr, $key_cost:expr) => {{
                nsw!();
                kvput_inner!($store, $key_cost);
            }};
        }

        macro_rules! kvget {
            ($k:ident => $lookup:expr) => {{
                nsr!();
                let v = {
                    let $k = ops.peek()?;
                    $lookup
                }
                .valid(cap)?;
                gas_resource!(stack_copy, v.dup_size());
                *ops.peek()? = v;
            }};
        }

        macro_rules! compo_edit_gas {
            () => {{
                let len = ops.compo()?.len();
                gas_resource!(compo_items_edit, len);
            }};
        }

        macro_rules! compo_read_gas {
            () => {{
                let len = ops.peek()?.container_len()?;
                gas_resource!(compo_items_read, len);
            }};
        }

        macro_rules! compo_pop_one {
            ($method:ident) => {{
                let mut compo_val = ops.pop()?;
                let len = compo_val.compo()?.len();
                let v = compo_val.compo()?.$method()?.valid(cap)?;
                gas_resource!(compo_items_edit, len);
                gas_resource!(compo_bytes, v.val_size());
                ops.push(v)?;
            }};
        }

        let step: VmrtRes<Step> = (|| {
            match instruction {
                // action
                ACTION | ACTENV | ACTVIEW => actcall!(instruction),
                // native runtime control (VM-local side effects)
                NTCTL => {
                    let nt_idx = pu8!();
                    let (r, g) = call_ntctl(exec, context_addr, deferred_registry, nt_idx)?;
                    finish_ntcall(cap, gst, &mut step_gas_use, ops, r, g)?;
                }
                // native func (pure computation, always allowed)
                NTFUNC => {
                    let nt_idx = pu8!();
                    let argv = ops.pop()?.extract_call_data(heap)?;
                    gas_resource!(nt_bytes, argv.len());
                    let (r, g) = call_ntfunc(hei, nt_idx, &argv)?;
                    finish_ntcall(cap, gst, &mut step_gas_use, ops, r, g)?;
                }
                // native env (VM context read, forbidden in Pure mode)
                NTENV  => {
                    let nt_idx = pu8!();
                    let (r, g) = call_ntenv(exec, context_addr, nt_idx)?;
                    finish_ntcall(cap, gst, &mut step_gas_use, ops, r, g)?;
                }
                // constant
                PU8 => ops.push(U8(pu8!()))?,
                PU16 => ops.push(U16(pu16!()))?,
                PBUF => push_buf_gas!(pbuf!()),
                PBUFL => push_buf_gas!(pbufl!()),
                P0 | P1 | P2 | P3 => ops.push(U8(instbyte - P0 as u8))?,
                PNBUF => ops.push(Value::empty_bytes())?,
                PNIL => ops.push(Value::Nil)?,
                PTRUE => ops.push(Bool(true))?,
                PFALSE => ops.push(Bool(false))?,
                // cast & type
                CU8 => ops.peek()?.cast_u8()?,
                CU16 => ops.peek()?.cast_u16()?,
                CU32 => ops.peek()?.cast_u32()?,
                CU64 => ops.peek()?.cast_u64()?,
                CU128 => ops.peek()?.cast_u128()?, /* CU256 => ops.peek()?.cast_u256()?, */
                CBUF => ops.peek()?.cast_buf()?,
                CTO => {
                    let ty = parse_cto_target_ty_param(pu8!())?;
                    ops.peek()?.cast_to(ty as u8)?;
                }
                TNIL => *ops.peek()? = Bool(pty!() == ValueTy::Nil),
                TLIST => *ops.peek()? = Bool(check_compo_type!(is_list)),
                TMAP => *ops.peek()? = Bool(check_compo_type!(is_map)),
                TIS => {
                    let ty = parse_value_ty_param(pu8!())?;
                    *ops.peek()? = Bool(pty!() == ty);
                }
                TID => *ops.peek()? = U8(ptyn!()),
                // stack & buffer
                DUP => {
                    let bsz = ops.datas.last().map(Value::dup_size).unwrap_or(0);
                    gas_resource!(stack_copy, bsz);
                    ops.push(ops.last()?)?;
                }
                DUPN => {
                    let n = pu8!();
                    let m = ops.datas.len();
                    let nsz = n as usize;
                    let mut bsz = 0usize;
                    if nsz <= m {
                        for v in &ops.datas[m - nsz..m] {
                            bsz += v.dup_size();
                        }
                    }
                    gas_resource!(stack_copy, bsz);
                    ops.dupn(n)?;
                }
                POP => {
                    ops.pop()?;
                } // drop
                POPN => {
                    ops.popn(pu8!())?;
                }
                ROLL0 => ops.roll(0)?,
                ROLL => ops.roll(pu8!())?,
                SWAP => ops.swap()?,
                REV => ops.reverse(pu8!())?, // reverse
                // CHOOSE: pop condition; if false swap the remaining two values so
                // the chosen branch becomes the top of the stack. Leave the
                // chosen value on the stack for subsequent instructions to consume.
                CHOOSE => {
                    if !ops.pop()?.extract_bool()? {
                        ops.swap()?
                    }
                    ops.pop()?;
                } /* x ? a : b */
                CAT => {
                    let (xlen, ylen) = match ops.datas.len() {
                        l if l >= 2 => (ops.datas[l - 2].val_size(), ops.datas[l - 1].val_size()),
                        _ => (0, 0),
                    };
                    gas_resource!(stack_op, xlen + ylen);
                    ops.cat(cap)?;
                }
                JOIN => {
                    let n = pu8!();
                    let total = {
                        let l = ops.datas.len();
                        let n = n as usize;
                        if n > l {
                            0
                        } else {
                            ops.datas[l - n..].iter().map(|v| v.val_size()).sum()
                        }
                    };
                    gas_resource!(stack_op, total);
                    ops.join(n, cap)?;
                }
                BYTE => {
                    let outlen = ops.peek()?.val_size();
                    gas_resource!(stack_op, outlen);
                    let i = ops_pop_to_u16!();
                    ops.peek()?.cutbyte(i)?;
                }
                CUT => {
                    let (l, o) = (ops.pop()?, ops.pop()?);
                    let outlen = ops.peek()?.val_size();
                    gas_resource!(stack_op, outlen);
                    ops.peek()?.cutout(l, o)?;
                }
                LEFT  => peek_op_gas!(cutleft(pu8_as_u16!())),
                RIGHT => peek_op_gas!(cutright(pu8_as_u16!())),
                LDROP => peek_op_gas!(dropleft(pu8_as_u16!())),
                RDROP => peek_op_gas!(dropright(pu8_as_u16!())),
                SIZE => *ops.peek()? = U16(ops.peek()?.can_get_size()?),
                // compo
                NEWLIST => ops.push(Compo(CompoItem::new_list()))?,
                NEWMAP => ops.push(Compo(CompoItem::new_map()))?,
                PACKTUPLE => {
                    let (a, len) = TupleItem::pack(cap, ops)?;
                    gas_resource!(compo_items_edit, len);
                    ops.push(a)?;
                }
                TUPLE2LIST => {
                    let (list, len, bsz) = match ops.peek()? {
                        Tuple(args) => match args.to_list_with_stats(cap) {
                            Ok(v) => v,
                            Err(ItrErr(CastBeValueFail, msg)) => {
                                return Err(ItrErr::new(CastFail, &msg))
                            }
                            Err(e) => return Err(e),
                        },
                        _ => return itr_err_code!(CastFail),
                    };
                    gas_resource!(compo_items_copy, len);
                    gas_resource!(compo_bytes, bsz);
                    *ops.peek()? = list;
                }
                PACKLIST => {
                    let (l, len) = CompoItem::pack_list(cap, ops)?;
                    gas_resource!(compo_items_edit, len);
                    ops.push(l)?;
                }
                PACKMAP => {
                    let (m, len) = CompoItem::pack_map(cap, ops)?;
                    gas_resource!(compo_items_edit, len);
                    ops.push(m)?;
                }
                INSERT => {
                    let v = ops.pop()?;
                    let k = ops.pop()?;
                    let ksz = {
                        let c = ops.compo()?;
                        maybe!(c.is_map(), k.extract_key_bytes()?.len(), 0)
                    };
                    compo_edit_gas!();
                    gas_resource!(compo_bytes, ksz);
                    gas_resource!(compo_bytes, v.val_size());
                    ops.compo()?.insert(cap, k, v)?;
                }
                REMOVE => {
                    let k = ops.pop()?;
                    compo_edit_gas!();
                    ops.compo()?.remove(k)?;
                }
                CLEAR => {
                    let (len, bsz) = {
                        let c = ops.compo()?;
                        (c.len(), c.val_size())
                    };
                    gas_resource!(compo_items_edit, len);
                    gas_resource!(compo_bytes, bsz);
                    ops.compo()?.clear();
                }
                MERGE => {
                    let a = ops.pop()?;
                    let (src_len, src_bsz) = match a.match_compo() {
                        Some(c) => {
                            let len = c.len();
                            let bsz = match c.list_ref() {
                                Ok(l) => l.iter().map(Value::val_size).sum(),
                                Err(_) => c
                                    .map_ref()?
                                    .iter()
                                    .map(|(k, v)| k.len() + v.val_size())
                                    .sum(),
                            };
                            (len, bsz)
                        }
                        None => (0, 0),
                    };
                    gas_resource!(compo_items_copy, src_len);
                    gas_resource!(compo_bytes, src_bsz);
                    ops.compo()?.merge(cap, a.take_compo()?)?;
                }
                LENGTH => {
                    let l = ops.peek()?.length(cap)?;
                    *ops.peek()? = l;
                }
                HASKEY => {
                    let k = ops.pop()?;
                    let h = ops.peek()?.haskey(k)?;
                    compo_read_gas!();
                    *ops.peek()? = h;
                }
                ITEMGET => {
                    let k = ops.pop()?;
                    let v = ops.peek()?.itemget(k)?.valid(cap)?;
                    compo_read_gas!();
                    gas_resource!(compo_bytes, v.val_size());
                    *ops.peek()? = v;
                }
                KEYS => {
                    let (v, len, bsz) = ops.compo()?.keys_with_stats()?;
                    gas_resource!(compo_items_read, len);
                    gas_resource!(compo_bytes, bsz);
                    *ops.peek()? = v;
                }
                VALUES => {
                    let (v, len, bsz) = ops.compo()?.values_with_stats()?;
                    gas_resource!(compo_items_read, len);
                    gas_resource!(compo_bytes, bsz);
                    *ops.peek()? = v;
                }
                HEAD => compo_pop_one!(head),
                BACK => compo_pop_one!(back),
                APPEND => {
                    let v = ops.pop()?;
                    compo_edit_gas!();
                    gas_resource!(compo_bytes, v.val_size());
                    ops.compo()?.append(cap, v)?;
                }
                CLONE => {
                    let (len, bsz, c) = {
                        let compo = ops.compo()?;
                        let len = compo.len();
                        let bsz = match compo.list_ref() {
                            Ok(l) => l.iter().map(Value::val_size).sum(),
                            Err(_) => compo
                                .map_ref()?
                                .iter()
                                .map(|(k, v)| k.len() + v.val_size())
                                .sum(),
                        };
                        (len, bsz, compo.copy())
                    };
                    gas_resource!(compo_items_copy, len);
                    gas_resource!(compo_bytes, bsz);
                    *ops.peek()? = Compo(c);
                }
                UNPACK => {
                    let i = ops.pop()?.extract_u8()?;
                    let items = ops.peek()?.clone_unpack_items()?;
                    gas_add!(resource, raw, unpack_seq(i, locals, items, gst, cap)?);
                    ops.pop()?; // pop argv wrapper after unpack
                }
                // heap
                HGROW => gas_add!(resource, raw, heap.grow(pu8!())?),
                HWRITE => hwrite!(ops_pop_to_u16!()),
                HREAD => {
                    let n = ops.pop()?;
                    let len = n.extract_u16()? as usize;
                    gas_resource!(heap_read, len);
                    let peek = ops.peek()?;
                    *peek = heap.read(peek, n)?.valid(cap)?;
                }
                HWRITEX => hwrite!(pu8_as_u16!()),
                HWRITEXL => hwrite!(pu16!()),
                HREADU => hread_push!(heap.read_u(pu8!())?),
                HREADUL => hread_push!(heap.read_ul(pu16!())?),
                HSLICE => {
                    let p = ops.pop()?;
                    let peek = ops.peek()?;
                    *peek = heap.slice(p, peek)?;
                }
                // locals & heap & global_map & memory_map
                XLG => {
                    let mark = pu8!();
                    let (opt, idx) = decode_local_logic_mark(mark);
                    if matches!(opt, LxLg::Eq | LxLg::Ne) {
                        let base = locals.load(idx as usize)?;
                        let top = ops.peek()?;
                        gas_resource!(stack_cmp, lgc_compare_fee(&base, top));
                    }
                    local_logic(mark, locals, ops.peek()?)?;
                }
                XOP => local_operand(pu8!(), locals, ops.pop()?)?,
                ALLOC => gas_add!(resource, raw, gst.one_local_alloc * locals.alloc(pu8!())? as i64),
                GETX => {
                    let v = locals.load(ops_peek_to_u16!() as usize)?.valid(cap)?;
                    gas_resource!(stack_copy, v.dup_size());
                    *ops.peek()? = v;
                }
                PUTX => {
                    let v = ops.pop()?.valid(cap)?;
                    let vlen = v.val_size();
                    gas_resource!(stack_write, vlen);
                    locals.save(ops_pop_to_u16!(), v)?;
                }
                PUT => {
                    let v = ops.pop()?.valid(cap)?;
                    let vlen = v.val_size();
                    gas_resource!(stack_write, vlen);
                    locals.save(pu8_as_u16!(), v)?;
                }
                GET => local_get!(pu8!()),
                GET0 | GET1 | GET2 | GET3 => local_get!(instbyte - GET0 as u8),
                // storage
                SREST => {
                    nsr!();
                    let v = {
                        let k = ops.peek()?;
                        host.srest(context_addr, k)?
                    }
                    .valid(cap)?;
                    *ops.peek()? = v;
                }
                SLOAD => {
                    nsr!();
                    let v = {
                        let k = ops.peek()?;
                        host.sload(context_addr, k)?
                    }
                    .valid(cap)?;
                    let vlen = v.val_size();
                    gas_add!(storage, storage_read, vlen);
                    *ops.peek()? = v;
                }
                SDEL => {
                    nsw!();
                    let k = ops.pop()?;
                    gas_add!(storage, storage_del);
                    host.sdel(context_addr, k)?;
                }
                SSAVE => {
                    nsw!();
                    let v = ops.pop()?.valid(cap)?;
                    let k = ops.pop()?;
                    gas_add!(storage, raw, host.ssave(gst, context_addr, k, v)?);
                }
                SRENT => {
                    nsw!();
                    let t = ops.pop()?;
                    let k = ops.pop()?;
                    gas_add!(storage, raw, host.srent(gst, context_addr, k, t)?);
                }
                // global_map & memory_map
                GPUT => kvput!(global_map, gst.global_key_cost),
                GGET => kvget!(k => global_map.get(k)?),
                MPUT => {
                    nsw!();
                    let mem = memory_map.entry_mut(context_addr)?;
                    kvput_inner!(mem, gst.memory_key_cost);
                }
                MGET => kvget!(k => memory_map.get(context_addr, k)?),
                // log (t1,[t2,t3,t4,]d)
                LOG1 | LOG2 | LOG3 | LOG4 => wlog!(instbyte - LOG1 as u8 + 2),
                // logic
                AND => binop_btw(ops, lgc_and)?,
                OR => binop_btw(ops, lgc_or)?,
                EQ => {
                    compare_gas!();
                    binop_btw(ops, lgc_equal)?
                }
                NEQ => {
                    compare_gas!();
                    binop_btw(ops, lgc_not_equal)?
                }
                LT => binop_btw(ops, lgc_less)?,
                GT => binop_btw(ops, lgc_greater)?,
                LE => binop_btw(ops, lgc_less_equal)?,
                GE => binop_btw(ops, lgc_greater_equal)?,
                NOT => ops.peek()?.cast_bool_not()?,
                // bitop
                BSHR => binop_arithmetic(ops, bit_shr)?,
                BSHL => binop_arithmetic(ops, bit_shl)?,
                BXOR => binop_arithmetic(ops, bit_xor)?,
                BOR => binop_arithmetic(ops, bit_or)?,
                BAND => binop_arithmetic(ops, bit_and)?,
                // arithmetic
                ADD => binop_arithmetic(ops, add_checked)?,
                SUB => binop_arithmetic(ops, sub_checked)?,
                MUL => binop_arithmetic(ops, mul_checked)?,
                DIV => binop_arithmetic(ops, div_checked)?,
                MOD => binop_arithmetic(ops, mod_checked)?,
                POW => binop_arithmetic(ops, pow_checked)?,
                MAX => binop_arithmetic(ops, max_checked)?,
                MIN => binop_arithmetic(ops, min_checked)?,
                DIVUP => binop_arithmetic(ops, divup_checked)?,
                DIVROUND => binop_arithmetic(ops, divround_checked)?,
                SATADD => binop_arithmetic(ops, satadd_checked)?,
                SATSUB => binop_arithmetic(ops, satsub_checked)?,
                ABSDIFF => binop_arithmetic(ops, absdiff_checked)?,
                ADDMOD => triop_arithmetic(ops, addmod_checked)?,
                MULMOD => triop_arithmetic(ops, mulmod_checked)?,
                MULDIV => triop_arithmetic(ops, muldiv_checked)?,
                MULADD => triop_arithmetic(ops, muladd_checked)?,
                MULSUB => triop_arithmetic(ops, mulsub_checked)?,
                MULDIVUP => triop_arithmetic(ops, muldivup_checked)?,
                MULDIVROUND => triop_arithmetic(ops, muldivround_checked)?,
                MULSHR => triop_arithmetic(ops, mulshr_checked)?,
                MULSHRUP => triop_arithmetic(ops, mulshrup_checked)?,
                RPOW => {
                    let exp_bits = match ops.len().checked_sub(2).and_then(|i| ops.datas.get(i)) {
                        Some(v) => {
                            let exp = v.to_uint()?.extract_u128()?;
                            if exp == 0 {
                                0
                            } else {
                                (u128::BITS - exp.leading_zeros()) as i64
                            }
                        }
                        None => 0,
                    };
                    gas_add!(compute, raw, exp_bits * 2 + 1);
                    triop_arithmetic(ops, rpow_checked)?
                }
                CLAMP => triop_arithmetic(ops, clamp_checked)?,
                DEVSCALED => triop_arithmetic(ops, devscaled_checked)?,
                MULADDDIV => quadop_arithmetic(ops, muladddiv_checked)?,
                MULSUBDIV => quadop_arithmetic(ops, mulsubdiv_checked)?,
                MUL3DIV => quadop_arithmetic(ops, mul3div_checked)?,
                WITHINBPS => quadop_arithmetic(ops, withinbps_checked)?,
                WAVG2 => quadop_arithmetic(ops, wavg2_checked)?,
                LERP => quadop_arithmetic(ops, lerp_checked)?,
                INC => unary_inc(ops.peek()?, pu8!())?,
                DEC => unary_dec(ops.peek()?, pu8!())?,
                // workflow control
                JMPL  => jump!(codes, *pc, 2),
                JMPS  => ostjump!(codes, *pc, 1),
                JMPSL => ostjump!(codes, *pc, 2),
                BRL   => branch!(ops, codes, *pc, 2),
                BRS   => ostbranch!(ops, codes, *pc, 1),
                BRSL  => ostbranch!(ops, codes, *pc, 2),
                BRSLN => ostbranchex!(ops, codes, *pc, 2, false),
                // other
                NT => return itr_err_code!(InstNeverTouch), // never touch
                NOP => {}                                   // do nothing
                BURN => {
                    gas_add!(resource, raw, pu16!());
                }
                // exit
                RET => return Ok(Step::Exit(Return)), // func return <DATA>
                END => return Ok(Step::Exit(Finish)), // func end
                ERR => return Ok(Step::Exit(Throw)),  // throw <ERROR>
                ABT => return Ok(Step::Exit(Abort)),  // panic
                AST => if !ops.pop()?.extract_bool()? {
                    return Ok(Step::Exit(Abort));
                } // assert(..)
                PRT => debug_print_value(context_addr, current_addr, exec, ops.pop()?),
                #[cfg(feature = "calcfunc")]
                CALCCALL => {
                    let end = *pc + FN_SIGN_WIDTH;
                    if end > codes.len() {
                        return itr_err_code!(CodeOverflow);
                    }
                    let selector = unsafe { read_arr::<FN_SIGN_WIDTH>(codes, *pc) };
                    *pc = end;
                    return Ok(Step::Exit(CalcCall(selector)));
                }
                // call
                CODECALL | CALL | CALLEXT | CALLEXTVIEW | CALLUSEVIEW | CALLUSEPURE | CALLTHIS | CALLSELF | CALLSUPER | CALLSELFVIEW | CALLSELFPURE => {
                    let plen = instruction.metadata().param as usize;
                    let end = *pc + plen;
                    if end > codes.len() {
                        return itr_err_code!(CodeOverflow);
                    }
                    let call = decode_user_call_site(instruction, &codes[*pc..end])?;
                    *pc = end;
                    check_call_mode(exec, &call)?;
                    return Ok(Step::Exit(Call(call)));
                }
                // inst invalid
                _ => return itr_err_fmt!(InstInvalid, "{}", instbyte),
            }
            Ok(Step::Continue)
        })();

        // reduce gas for use
        let step_total = check_add_gas_use(gas_use, &step_gas_use, gst)?;
        host.gas_charge(step_total)?;
        match step {
            Ok(Step::Exit(exit)) => return Ok(exit),
            Ok(Step::Continue) => {}
            Err(e) => return Err(e),
        }
        // next
    }
}


#[inline(always)]
fn check_add_gas_use(
    gas_use: &mut GasUse,
    step_gas_use: &GasUse,
    gst: &GasExtra,
) -> VmrtRes<i64> {
    if step_gas_use.compute < 0 || step_gas_use.resource < 0 || step_gas_use.storage < 0 {
        return itr_err_fmt!(
            ItrErrCode::GasError,
            "gas cost invalid: compute={}, resource={}, storage={}",
            step_gas_use.compute,
            step_gas_use.resource,
            step_gas_use.storage
        );
    }
    let step_total = step_gas_use
        .checked_total()
        .ok_or_else(|| ItrErr::new(ItrErrCode::OutOfGas, "gas overflow"))?;
    let next_gas_use = gas_use
        .checked_add(step_gas_use)
        .ok_or_else(|| ItrErr::new(ItrErrCode::OutOfGas, "gas use overflow"))?;
    let check_limit = |name: &str, used: i64, limit: i64| -> VmrtErr {
        if limit > 0 && used > limit {
            return itr_err_fmt!(
                ItrErrCode::OutOfGas,
                "{} gas limit exceeded: used {} > limit {}",
                name,
                used,
                limit
            );
        }
        Ok(())
    };
    check_limit("compute",  next_gas_use.compute,  gst.compute_limit)?;
    check_limit("resource", next_gas_use.resource, gst.resource_limit)?;
    check_limit("storage",  next_gas_use.storage,  gst.storage_limit)?;
    *gas_use = next_gas_use;
    Ok(step_total)
}

#[allow(unused)]
fn debug_print_stack(ops: &Stack, lcs: &Stack, pc: &usize, inst: Bytecode) {
    println!(
        "operds({})={}\nlocals({})={}\n-------- pc = {}, nbt = {:?}",
        ops.len(),
        &ops.print_stack(),
        lcs.len(),
        &lcs.print_stack(),
        *pc,
        inst
    );
}


#[allow(unused)]
fn debug_print_value(
    _ctx: &field::Address,
    _cur: &field::Address,
    _exec: ExecCtx,
    _val: Value,
) {
    debug_println!(
        "{}-{} {} {:?} => {:?}",
        _ctx.prefix(7),
        _cur.prefix(7),
        _exec.call_depth,
        _exec,
        _val
    );
}
