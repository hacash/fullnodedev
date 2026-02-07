
/**
* parse bytecode params
*/

use crate::machine::VmHost;

#[inline(always)]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe fn read_arr<const N: usize>(codes: &[u8], pc: usize) -> [u8; N] {
    let mut out = [0u8; N];
    std::ptr::copy_nonoverlapping(codes.as_ptr().add(pc), out.as_mut_ptr(), N);
    out
}



macro_rules! itrbuf {
    ($codes: expr, $pc: expr, $l: expr) => {
        { 
            let r = $pc + $l;
            if r < $pc || r > $codes.len() {
                return itr_err_code!(CodeOverflow)
            }
            let v: [u8; $l] = unsafe { read_arr::<$l>($codes, $pc) };
            $pc = r;
            v
        }
    }
}

macro_rules! itrparam {
    ($codes: expr, $pc: expr, $l: expr, $t: ty) => {
        { 
            let r = $pc + $l;
            if r < $pc || r > $codes.len() {
                return itr_err_code!(CodeOverflow)
            }
            let v = <$t>::from_be_bytes(unsafe { read_arr::<$l>($codes, $pc) });
            $pc = r;
            v
        }
    }
}

macro_rules! itrparamu8 {
    ($codes: expr, $pc: expr) => {
        itrparam!{$codes, $pc, 1, u8}
    }
}

macro_rules! itrparamu16 {
    ($codes: expr, $pc: expr) => {
        itrparam!{$codes, $pc, 2, u16}
    }
}

macro_rules! itrparambufex {
    ($codes: expr, $pc: expr, $l: expr, $t: ty) => {
        {
            let s = itrparam!{$codes, $pc, $l, $t} as usize;
            let l = $pc;
            let r = l + s;
            if r < l || r > $codes.len() {
                return itr_err_code!(CodeOverflow)
            }
            $pc = r;
            let v = unsafe { std::slice::from_raw_parts($codes.as_ptr().add(l), s) };
            Value::Bytes(v.to_vec())
        }
    }
}

macro_rules! itrparambuf {
    ($codes: expr, $pc: expr) => {
        itrparambufex!($codes, $pc, 1, u8)
    }
}

macro_rules! itrparambufl {
    ($codes: expr, $pc: expr) => {
        itrparambufex!($codes, $pc, 2, u16)
    }
}

macro_rules! jump {
    ($codes: expr, $pc: expr, $l: expr) => {
        {
            let tpc = match $l {
                1 =>  itrparamu8!($codes, $pc) as usize,
                2 => itrparamu16!($codes, $pc) as usize,
                _ => return itr_err_code!(CodeOverflow),
            };
            $pc = tpc; // jump to
        }
    }
}

macro_rules! ostjump {
    ($codes: expr, $pc: expr, $l: expr) => {
        {
            let tpc = match $l {
                1 => itrparam!{$codes, $pc, 1, i8} as isize,
                2 => itrparam!{$codes, $pc, 2, i16} as isize,
                _ => return itr_err_code!(CodeOverflow),
            };
            let tpc = ($pc as isize + tpc);
            if tpc < 0 {
                return itr_err_code!(CodeOverflow)
            }
            $pc = tpc as usize; // jump to
        }
    }
}

macro_rules! branch {
    ( $ops: expr, $codes: expr, $pc: expr, $l: expr) => {
        if $ops.pop()?.check_true() {
            jump!($codes, $pc, $l);
        }else{
            $pc += $l;
        }
    }
}

macro_rules! ostbranchex {
    ( $ops: expr, $codes: expr, $pc: expr, $l: expr, $cond: ident) => {
        if $ops.pop()?.$cond() {
            ostjump!($codes, $pc, $l);
        }else{
            $pc += $l;
        }
    }
}
// is_not_zero
macro_rules! ostbranch {
    ( $ops: expr, $codes: expr, $pc: expr, $l: expr) => {
        ostbranchex!($ops, $codes, $pc, $l, check_true)
    }
}

macro_rules! funcptr {
    ($codes: expr, $pc: expr, $mode: expr) => {
        {
            let idx = itrparamu8!($codes, $pc);
            let sig = itrbuf!($codes, $pc, FN_SIGN_WIDTH);
            Call(Funcptr{
                mode: $mode,
                is_callcode: false,
                target: CallTarget::Libidx(idx),
                fnsign: sig,
            })
        }
    }
}


/**
* execute code
*/
pub fn execute_code(

    pc: &mut usize, // pc
    codes: &[u8], // max len = 65536
    mode: ExecMode,
    in_callcode: bool,
    depth: isize,

    gas_usable: &mut i64, // max gas can be use

    gas_table: &GasTable, // len = 256
    gas_extra: &GasExtra,
    space_cap: &SpaceCap,

    operands: &mut Stack,
    locals: &mut Stack,
    heap: &mut Heap,

    globals: &mut GKVMap,
    memorys: &mut CtcKVMap,

    host: &mut dyn VmHost,

    context_addr: &ContractAddress, 
    current_addr: &ContractAddress, 

    // _is_sys_call: bool,
    // _call_depth: usize,

) -> VmrtRes<CallExit> {

    use Value::*;
    use ExecMode::*;
    use CallExit::*;
    use ItrErrCode::*;
    use Bytecode::*;

    let cap = space_cap;
    let ops = operands;
    let gst = gas_extra;
    let hei: u64 = host.height();

    // check code length
    // let codelen = codes.len();
    // let tail = codelen;

    macro_rules! check_gas { () => { if *gas_usable < 0 { return itr_err_code!(OutOfGas) } } }
    macro_rules! nsr { () => { if let Pure        = mode { return itr_err_code!(InstDisabled) } } } // not read  in pure mode
    macro_rules! nsw { () => { if let Pure | View = mode { return itr_err_code!(InstDisabled) } } } // not write in view/pure mode
    macro_rules! pu8 { () => { itrparamu8!(codes, *pc) } }
    macro_rules! pty { () => { ops.peek()?.ty() } }
    macro_rules! ptyn { () => { ops.peek()?.ty_num() } }
    macro_rules! pu8_as_u16 { () => { pu8!() as u16 } }
    macro_rules! pu16 { () => { itrparamu16!(codes, *pc) } }
    macro_rules! pbuf { () => { itrparambuf!(codes, *pc) } }
    macro_rules! pbufl { () => { itrparambufl!(codes, *pc) } }
    macro_rules! pcutbuf { ($w: expr) => { itrbuf!(codes, *pc, $w) } }
    macro_rules! _pctrtaddr { () => { ContractAddress::parse(&pcutbuf!(CONTRACT_ADDRESS_WIDTH)).map_err(|e|ItrErr(ContractAddrErr, e))? }}
    macro_rules! ops_pop_to_u16 { () => { ops.pop()?.checked_u16()? } }
    macro_rules! ops_peek_to_u16 { () => { ops.peek()?.checked_u16()? } }
    macro_rules! check_compo_type { ($m: ident) => { match ops.compo() { Ok(c) => c.$m(), _ => false, } } }

    // start run
    let exit;
	    loop {
	        // read inst
	        if *pc >= codes.len() {
	            return itr_err_code!(CodeOverflow)
	        }
	        let instbyte = unsafe { *codes.get_unchecked(*pc as usize) }; // u8
	        let instruction: Bytecode = std_mem_transmute!(instbyte);
	        *pc += 1; // next

        // debug_print_stack(ops, locals, pc, instruction);

        // do execute
        let mut gas: i64 = 0;
        *gas_usable -= gas_table.gas(instbyte); // 
        // println!("gas usable {} cp: {}, inst: {:?}", *gas_usable, gas_table.gas(instbyte), instruction);

	        macro_rules! extcall { ($act_kind: expr, $pass_body: expr, $have_retv: expr) => {
            if in_callcode && EXTACTION == $act_kind {
                return itr_err_fmt!(ExtActDisabled, "extend action not allowed in callcode")
            }
            if EXTACTION == $act_kind && (mode != Main || depth > 0)  {
                return itr_err_fmt!(ExtActDisabled, "extend action just can use in main call")
            }
            let idx = pu8!();
            ensure_extend_call_allowed(mode, $act_kind, idx)?;
            let kid = u16::from_be_bytes([instbyte, idx]);
	            let mut actbody = vec![];
	            if $pass_body {
	                let mut bdv = ops.peek()?.canbe_ext_call_data(heap)?;
	                actbody.append(&mut bdv);
                    match $act_kind {
                        EXTACTION => gas += gst.extaction_bytes(actbody.len()),
                        EXTFUNC => gas += gst.extfunc_bytes(actbody.len()),
                        _ => {}
                    }
	            }
	            let (bgasu, cres) = host.ext_action_call(kid, actbody).map_err(|e|
	                ItrErr::new(ExtActCallError, e.as_str()))?;
	            gas += bgasu as i64;
	            if $have_retv {
	                let vty = match instruction {
	                    EXTENV  => search_ext_by_id(idx, &CALL_EXTEND_ENV_DEFS),
	                    EXTFUNC => search_ext_by_id(idx, &CALL_EXTEND_FUNC_DEFS),
	                    _ => never!(),
	                }.ok_or_else(|| ItrErr::new(ExtActCallError, &format!("extend id {} not found", idx)))?.2;
	                let resv = Value::type_from(vty, cres)?.valid(cap)?; // from ty + stack size bound
                    match $act_kind {
                        EXTFUNC => gas += gst.extfunc_bytes(bytes_len(&resv)),
                        _ => {}
                    }
	                if $pass_body {
	                    *ops.peek()? = resv;
	                } else {
	                    ops.push(resv)?;
	                }
	            } else if $pass_body {
	                // EXTACTION: returns bytes but does not keep it on stack
	                ops.pop()?;
	            } else {
	                never!()
	            }
	        }}

	        let mut ntcall = |idx: u8| -> VmrtErr {
            // Special native calls that read execution/state-like context should follow the same
            // read privilege as state reads (nsr!): disallow in Pure(callpure).
	                let mut argv = match idx {
                NativeCall::idx_context_address => context_addr.serialize(), // context_address
                _ => vec![],
            };
	            let argl = NativeCall::args_len(idx);
	            if argl > 0 {
	                argv = ops.peek()?.canbe_ext_call_data(heap)?;
                    gas += gst.ntcall_bytes(argv.len());
	            } else {
	                nsr!{};
	            }
	            let (r, g) = NativeCall::call(hei, idx, &argv)?;
	            let r = r.valid(cap)?;
                gas += gst.ntcall_bytes(bytes_len(&r));
	            if argl > 0 {
	                *ops.peek()? = r; 
	            } else {
	                ops.push(r)?;
	            }
            gas += g;
            Ok(())
        };

        match instruction {
            // ext action
            EXTACTION => { extcall!(EXTACTION, true,  false); },
            EXTENV    => { extcall!(EXTENV,    false, true);  },
            EXTFUNC   => { extcall!(EXTFUNC,   true,  true);  },
            // native call
            NTCALL => ntcall(pu8!())?,
            // constant
            PU8   => ops.push(U8(pu8!()))?,
            PU16  => ops.push(U16(pu16!()))?,
            PBUF  => {
                let v = pbuf!().valid(cap)?;
                if let Value::Bytes(b) = &v {
                    gas += gst.stack_copy(b.len());
                }
                ops.push(v)?;
            }
            PBUFL => {
                let v = pbufl!().valid(cap)?; // buf long
                if let Value::Bytes(b) = &v {
                    gas += gst.stack_copy(b.len());
                }
                ops.push(v)?;
            }
            P0    => ops.push(U8(0))?,
            P1    => ops.push(U8(1))?,
            P2    => ops.push(U8(2))?,
            P3    => ops.push(U8(3))?,
            PNBUF => ops.push(Value::empty_bytes())?,
            PNIL  => ops.push(Value::Nil)?,
            // cast & type
            CU8   => ops.peek()?.cast_u8()?,
            CU16  => ops.peek()?.cast_u16()?,
            CU32  => ops.peek()?.cast_u32()?,
            CU64  => ops.peek()?.cast_u64()?,
            CU128 => ops.peek()?.cast_u128()?, /* CU256 => ops.peek()?.cast_u256()?, */
            CBUF  => ops.peek()?.cast_buf()?,
            CTO   => ops.peek()?.cast_to(pu8!())?,
            TNIL  => *ops.peek()? = Bool(pty!() == ValueTy::Nil),
            TLIST => *ops.peek()? = Bool(check_compo_type!(is_list)),
            TMAP  => *ops.peek()? = Bool(check_compo_type!(is_map)),
            TIS   => *ops.peek()? = Bool(ptyn!() == pu8!()),
            TID   => *ops.peek()? =   U8(ptyn!()),
            // stack & buffer
            DUP    => {
                let bsz = ops.datas.last().map(bytes_len).unwrap_or(0);
                ops.push(ops.last()?)?;
                gas += gst.stack_copy(bsz);
            }
            DUPN   => {
                let n = pu8!();
                let m = ops.datas.len();
                let nsz = n as usize;
                let mut bsz = 0usize;
                if nsz <= m {
                    for v in &ops.datas[m - nsz..m] {
                        bsz += bytes_len(v);
                    }
                }
                ops.dupn(n)?;
                gas += gst.stack_copy(bsz);
            }
            POP    => { ops.pop()?; } // drop
            POPN   => { ops.popn(pu8!())?; },
            PICK   => ops.pick(pu8!())?,
            SWAP   => ops.swap()?,
            REV    => ops.reverse(pu8!())?, // reverse
            CHOISE => { if ops.pop()?.check_false() { ops.swap()? } ops.pop()?; } /* x ? a : b */
            CAT    => {
                let (xlen, ylen) = match ops.datas.len() {
                    l if l >= 2 => (ops.datas[l - 2].val_size(), ops.datas[l - 1].val_size()),
                    _ => (0, 0),
                };
                ops.cat(cap)?;
                gas += gst.stack_copy(xlen + ylen);
            }
            JOIN   => {
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
                ops.join(n, cap)?;
                gas += gst.stack_copy(total);
            }
            BYTE   => {
                let i = ops_pop_to_u16!();
                ops.peek()?.cutbyte(i)?;
                let outlen = ops.peek()?.val_size();
                gas += gst.stack_copy(outlen);
            }
            CUT    => {
                let (l, o) = (ops.pop()?, ops.pop()?);
                ops.peek()?.cutout(l, o)?;
                let outlen = ops.peek()?.val_size();
                gas += gst.stack_copy(outlen);
            }
            LEFT   => {
                ops.peek()?.cutleft(pu8_as_u16!())?;
                let outlen = ops.peek()?.val_size();
                gas += gst.stack_copy(outlen);
            }
            RIGHT  => {
                ops.peek()?.cutright(pu8_as_u16!())?;
                let outlen = ops.peek()?.val_size();
                gas += gst.stack_copy(outlen);
            }
            LDROP  => {
                ops.peek()?.dropleft(pu8_as_u16!())?;
                let outlen = ops.peek()?.val_size();
                gas += gst.stack_copy(outlen);
            }
            RDROP  => {
                ops.peek()?.dropright(pu8_as_u16!())?;
                let outlen = ops.peek()?.val_size();
                gas += gst.stack_copy(outlen);
            }
            SIZE   => { *ops.peek()? = U16(ops.peek()?.can_get_size()?) }
            // compo
            NEWLIST  => ops.push(Compo(CompoItem::new_list()))?,
            NEWMAP   => ops.push(Compo(CompoItem::new_map()))?,
            PACKLIST => {
                let l = CompoItem::pack_list(cap, ops)?;
                ops.push(l)?;
            }
            PACKMAP  => {
                let m = CompoItem::pack_map(cap, ops)?;
                ops.push(m)?;
            }
            INSERT   => {
                let v = ops.pop()?;
                let k = ops.pop()?;
                let len = ops.compo()?.len();
                ops.compo()?.insert(cap, k, v)?;
                gas += gst.compo_items(len, 2);
            }
            REMOVE   => {
                let k = ops.pop()?;
                let len = ops.compo()?.len();
                ops.compo()?.remove(k)?;
                gas += gst.compo_items(len, 2);
            }
            CLEAR    => { ops.compo()?.clear() }
            MERGE    => {
                let a = ops.pop()?;
                let (src_len, src_bsz) = match &a {
                    Value::Compo(c) => {
                        let len = c.len();
                        let bsz = match c.list_ref() {
                            Ok(l) => l.iter().map(|v| bytes_len(v)).sum(),
                            Err(_) => c
                                .map_ref()?
                                .iter()
                                .map(|(k, v)| k.len() + bytes_len(v))
                                .sum(),
                        };
                        (len, bsz)
                    }
                    _ => (0, 0),
                };
                ops.compo()?.merge(cap, a.compo_get()?)?;
                gas += gst.compo_items(src_len, 1);
                gas += gst.compo_bytes(src_bsz);
            }
            LENGTH   => { let l = ops.compo()?.length(cap)?; *ops.peek()? = l; }
            HASKEY   => {
                let k = ops.pop()?;
                let len = ops.compo()?.len();
                let h = ops.compo()?.haskey(k)?;
                *ops.peek()? = h;
                gas += gst.compo_items(len, 4);
            }
	        ITEMGET  => {
                let k = ops.pop()?;
                let len = ops.compo()?.len();
                let v = ops.compo()?.itemget(k)?.valid(cap)?;
                gas += gst.compo_items(len, 4);
                gas += gst.compo_bytes(bytes_len(&v));
                *ops.peek()? = v;
            }
            KEYS     => {
                let len = { ops.compo()?.len() };
                let bsz = { let c = ops.compo()?; c.map_ref()?.keys().map(|k| k.len()).sum() };
                let v = { let c = ops.compo()?; c.keys()? };
                *ops.peek()? = v;
                gas += gst.compo_items(len, 2);
                gas += gst.compo_bytes(bsz);
            }
            VALUES   => {
                let len = { ops.compo()?.len() };
                let bsz = {
                    let c = ops.compo()?;
                    c.map_ref()?
                        .values()
                        .map(|v| bytes_len(v))
                        .sum()
                };
                let v = { let c = ops.compo()?; c.values()? };
                *ops.peek()? = v;
                gas += gst.compo_items(len, 2);
                gas += gst.compo_bytes(bsz);
            }
            HEAD     => {
                let mut compo_val = ops.pop()?;
                let len = compo_val.compo()?.len();
                let v = compo_val.compo()?.head()?.valid(cap)?;
                gas += gst.compo_items(len, 4);
                gas += gst.compo_bytes(bytes_len(&v));
                ops.push(v)?;
            }
            TAIL     => {
                let mut compo_val = ops.pop()?;
                let len = compo_val.compo()?.len();
                let v = compo_val.compo()?.tail()?.valid(cap)?;
                gas += gst.compo_items(len, 4);
                gas += gst.compo_bytes(bytes_len(&v));
                ops.push(v)?;
            }
            APPEND   => {
                let v = ops.pop()?;
                let len = ops.compo()?.len();
                ops.compo()?.append(cap, v)?;
                gas += gst.compo_items(len, 4);
            }
            CLONE    => {
                let (len, bsz, c) = {
                    let compo = ops.compo()?;
                    let len = compo.len();
                    let bsz = match compo.list_ref() {
                        Ok(l) => l.iter().map(|v| bytes_len(v)).sum(),
                        Err(_) => compo
                            .map_ref()?
                            .iter()
                            .map(|(k, v)| k.len() + bytes_len(v))
                            .sum(),
                    };
                    (len, bsz, compo.copy())
                };
                *ops.peek()? = Compo(c);
                gas += gst.compo_items(len, 1);
                gas += gst.compo_bytes(bsz);
            }
            UPLIST   => {
                let i = ops.pop()?.checked_u8()?;
                let list_len = { let c = ops.compo()?; c.list_ref()?.len() };
                unpack_list(i, locals, ops.compo()?.list_ref()?)?;
                ops.pop()?;
                gas += gst.compo_items(list_len, 4);
            }
            // heap
            HGROW    => gas += heap.grow(pu8!())?,
            HWRITE   => {
                let v = ops.pop()?;
                let vlen = v.val_size();
                heap.write(ops_pop_to_u16!(), v)?;
                gas += gst.heap_write(vlen);
            }
            HREAD    => {
                let n = ops.pop()?;
                let len = n.checked_u16()? as usize;
                let peek = ops.peek()?;
                *peek = heap.read(peek, n)?.valid(cap)?;
                gas += gst.heap_read(len);
            }
            HWRITEX  => {
                let v = ops.pop()?;
                let vlen = v.val_size();
                heap.write(pu8_as_u16!(), v)?;
                gas += gst.heap_write(vlen);
            }
            HWRITEXL => {
                let v = ops.pop()?;
                let vlen = v.val_size();
                heap.write(pu16!(), v)?;
                gas += gst.heap_write(vlen);
            }
            HREADU   => ops.push(heap.read_u(  pu8!())?)?,
            HREADUL  => ops.push(heap.read_ul(pu16!())?)?,
            HSLICE   => { let p = ops.pop()?; let peek = ops.peek()?; *peek = heap.slice(p, peek)?; }
            // locals & heap & global & memory
            XLG   => local_logic(pu8!(), locals, ops.peek()?)?,
            XOP   => local_operand(pu8!(), locals, ops.pop()?)?,
            ALLOC => { gas += gst.local_one_alloc * locals.alloc(pu8!())? as i64 } 
	            PUTX   => { let v = ops.pop()?.valid(cap)?; locals.save(ops_pop_to_u16!(), v)? }
	            GETX   => {
	                let v = locals.load(ops_peek_to_u16!() as usize)?.valid(cap)?;
	                gas += gst.stack_copy(bytes_len(&v));
	                *ops.peek()? = v;
	            }
	            PUT   => locals.save(pu8_as_u16!(), ops.pop()?.valid(cap)?)?,
	            GET   => {
	                let v = locals.load(pu8!() as usize)?.valid(cap)?;
	                gas += gst.stack_copy(bytes_len(&v));
	                ops.push(v)?;
	            }
	            GET0  => {
	                let v = locals.load(0)?.valid(cap)?;
	                gas += gst.stack_copy(bytes_len(&v));
	                ops.push(v)?;
	            }
	            GET1  => {
	                let v = locals.load(1)?.valid(cap)?;
	                gas += gst.stack_copy(bytes_len(&v));
	                ops.push(v)?;
	            }
	            GET2  => {
	                let v = locals.load(2)?.valid(cap)?;
	                gas += gst.stack_copy(bytes_len(&v));
	                ops.push(v)?;
	            }
	            GET3  => {
	                let v = locals.load(3)?.valid(cap)?;
	                gas += gst.stack_copy(bytes_len(&v));
	                ops.push(v)?;
	            }
            // storage
            SREST => {
                nsr!();
                let v = { let k = ops.peek()?; host.srest(hei, context_addr, k)? }.valid(cap)?;
                let vlen = v.val_size();
                *ops.peek()? = v;
                gas += gst.storage_read(vlen);
            }
            SLOAD => {
                nsr!();
                let v = { let k = ops.peek()?; host.sload(hei, context_addr, k)? }.valid(cap)?;
                let vlen = v.val_size();
                *ops.peek()? = v;
                gas += gst.storage_read(vlen);
            }
            SDEL  => {
                nsw!();
                let k = ops.pop()?;
                host.sdel(context_addr, k)?;
                gas += gst.storage_del();
            }
            SSAVE => {
                nsw!();
                let v = ops.pop()?;
                let k = ops.pop()?;
                gas += host.ssave(gst, hei, context_addr, k, v)?;
            }
            SRENT => { nsw!(); let t = ops.pop()?; let k = ops.pop()?; gas += host.srent(gst, hei, context_addr, k, t)?; }
            // global & memory
            GPUT => {
                nsw!();
                let v = ops.pop()?.valid(cap)?;
                let k = ops.pop()?;
                let is_new = !globals.contains_key(&k)?;
                globals.put(k, v)?;
                if is_new {
                    gas += gst.global_key_cost;
                }
            }
            GGET => {
                nsr!();
                let v = { let k = ops.peek()?; globals.get(k)? }.valid(cap)?;
                gas += gst.stack_copy(bytes_len(&v));
                *ops.peek()? = v;
            }
            MPUT => {
                nsw!();
                let v = ops.pop()?.valid(cap)?;
                let k = ops.pop()?;
                let mem = memorys.entry(context_addr)?;
                let is_new = !mem.contains_key(&k)?;
                mem.put(k, v)?;
                if is_new {
                    gas += gst.memory_key_cost;
                }
            }
            MGET => {
                nsr!();
                let v = { let k = ops.peek()?; memorys.entry(context_addr)?.get(k)? }.valid(cap)?;
                gas += gst.stack_copy(bytes_len(&v));
                *ops.peek()? = v;
            }
            // log (t1,[t2,t3,t4,]d)
            LOG1 => { nsw!(); let items = ops.popn(2)?; gas += gst.log_bytes(items.iter().map(|v| v.val_size()).sum()); host.log_push(context_addr, items)?; }
            LOG2 => { nsw!(); let items = ops.popn(3)?; gas += gst.log_bytes(items.iter().map(|v| v.val_size()).sum()); host.log_push(context_addr, items)?; }
            LOG3 => { nsw!(); let items = ops.popn(4)?; gas += gst.log_bytes(items.iter().map(|v| v.val_size()).sum()); host.log_push(context_addr, items)?; }
            LOG4 => { nsw!(); let items = ops.popn(5)?; gas += gst.log_bytes(items.iter().map(|v| v.val_size()).sum()); host.log_push(context_addr, items)?; }
            // logic
            AND  => binop_btw(ops, lgc_and)?,
            OR   => binop_btw(ops, lgc_or)?,
            EQ   => binop_btw(ops, lgc_equal)?,
            NEQ  => binop_btw(ops, lgc_not_equal)?,
            LT   => binop_btw(ops, lgc_less)?,
            GT   => binop_btw(ops, lgc_greater)?,
            LE   => binop_btw(ops, lgc_less_equal)?,
            GE   => binop_btw(ops, lgc_greater_equal)?,
            NOT  => ops.peek()?.cast_bool_not()?,
            // bitop
            BSHR => binop_arithmetic(ops, bit_shr)?,
            BSHL => binop_arithmetic(ops, bit_shl)?,
            BXOR => binop_arithmetic(ops, bit_xor)?,
            BOR  => binop_arithmetic(ops, bit_or)?,
            BAND => binop_arithmetic(ops, bit_and)?,
            // arithmetic
            ADD  => binop_arithmetic(ops, add_checked)?,
            SUB  => binop_arithmetic(ops, sub_checked)?,
            MUL  => binop_arithmetic(ops, mul_checked)?,
            DIV  => binop_arithmetic(ops, div_checked)?,
            MOD  => binop_arithmetic(ops, mod_checked)?,
            POW  => binop_arithmetic(ops, pow_checked)?,
            MAX  => binop_arithmetic(ops, max_checked)?,
            MIN  => binop_arithmetic(ops, min_checked)?,
            INC  => ops.peek()?.inc(pu8!())?,
            DEC  => ops.peek()?.dec(pu8!())?,
            // workflow control
            JMPL  =>        jump!(codes, *pc, 2),
            JMPS  =>     ostjump!(codes, *pc, 1),
            JMPSL =>     ostjump!(codes, *pc, 2),
            BRL   =>      branch!(ops, codes, *pc, 2),
            BRS   =>   ostbranch!(ops, codes, *pc, 1),
            BRSL  =>   ostbranch!(ops, codes, *pc, 2),   
            BRSLN => ostbranchex!(ops, codes, *pc, 2, check_false),   
            // other
            NT   => return itr_err_code!(InstNeverTouch), // never touch
            NOP  => {}, // do nothing
            BURN => gas += pu16!() as i64,         
            // exit
            RET => { exit = Return; break }, // func return <DATA>
            END => { exit = Finish; break }, // func end
            ERR => { exit = Throw;  break },  // throw <ERROR>
            ABT => { exit = Abort;  break },  // panic
            AST => { if ops.pop()?.check_false() { exit = Abort;  break } }, // assert(..)
            PRT => { debug_print_value(context_addr, current_addr, mode, depth, ops.pop()?) }
            // call CALLDYN
            CALLCODE | CALLPURE | CALLVIEW | CALLTHIS | CALLSELF | CALLSUPER | CALL => {
                let ist = instruction;
                check_call_mode(mode, ist, in_callcode)?;
                // ok return
                match ist {
                    CALLCODE => {
                        // CALLCODE inherits current mode permissions, and marks in_callcode
                        let idx = itrparamu8!(codes, *pc);
                        let sig = itrbuf!(codes, *pc, FN_SIGN_WIDTH);
                        exit = Call(Funcptr{
                            mode,
                            is_callcode: true,
                            target: CallTarget::Libidx(idx),
                            fnsign: sig,
                        })
                    },
                    CALLPURE => exit = funcptr!(codes, *pc, Pure),
                    CALLVIEW => exit = funcptr!(codes, *pc, View),
                    CALL =>       exit = funcptr!(codes, *pc, Outer),
                    CALLTHIS =>   exit = Call(Funcptr{
                        mode: Inner,
                        is_callcode: false,
                        target: CallTarget::This,
                        fnsign: pcutbuf!(FN_SIGN_WIDTH),
                    }),
                    CALLSELF =>   exit = Call(Funcptr{
                        mode: Inner,
                        is_callcode: false,
                        target: CallTarget::Self_,
                        fnsign: pcutbuf!(FN_SIGN_WIDTH),
                    }),
                    CALLSUPER =>  exit = Call(Funcptr{
                        mode: Inner,
                        is_callcode: false,
                        target: CallTarget::Super,
                        fnsign: pcutbuf!(FN_SIGN_WIDTH),
                    }),
                    /* CALLDYN =>    exit = Call(Funcptr{ // Outer
                        mode: Outer,
                        target: CallTarget::Addr(ops.pop()?.checked_contract_address()?),
                        fnsign: ops.pop()?.checked_fnsign()?,
                    }), */
                    _ => unreachable!()
                };
                break
                // call exit
            }
            // inst invalid
            _ => return itr_err_fmt!(InstInvalid, "{}", instbyte),
        }

        // reduce gas for use
        *gas_usable -= gas; // more gas use
        check_gas!();
        // next
    }

    // exit
    check_gas!();
    Ok(exit)

}


fn check_call_mode(mode: ExecMode, inst: Bytecode, in_callcode: bool) -> VmrtErr {
    use ExecMode::*;
    use Bytecode::*;
    if in_callcode {
        // In CALLCODE execution, no further call instructions are allowed.
        return itr_err_code!(CallInCallcode)
    }
    macro_rules! not_ist {
        ( $( $ist: expr ),+ ) => {
            ![$( $ist ),+].contains(&inst)
        }
    }
    match mode {
        Main    if not_ist!(CALL, CALLVIEW,   CALLPURE,   CALLCODE) => itr_err_code!(CallOtherInMain),
        P2sh    if not_ist!(         CALLVIEW, CALLPURE,   CALLCODE) => itr_err_code!(CallOtherInP2sh),
        Abst    if not_ist!(CALLTHIS, CALLSELF, CALLSUPER, CALLVIEW, CALLPURE, CALLCODE) => itr_err_code!(CallInAbst),
        View    if not_ist!(         CALLVIEW, CALLPURE,  CALLCODE) => itr_err_code!(CallLocInView),
        Pure    if not_ist!(                  CALLPURE            ) => itr_err_code!(CallInPure),
        _ => Ok(()), // Outer | Inner support all call instructions
    }
}


fn local_operand(mark: u8, locals: &mut Stack, mut value: Value) -> VmrtErr {
    let opt = mark >> 6; // 0b00000011
    let idx = mark & 0b00111111; // max=64
    let basev = locals.edit(idx)?;
    match opt {
        0 => locop_arithmetic(basev, &mut value, add_checked), // +=
        1 => locop_arithmetic(basev, &mut value, sub_checked), // -=
        2 => locop_arithmetic(basev, &mut value, mul_checked), // *=
        3 => locop_arithmetic(basev, &mut value, div_checked), // /=
        _ => unreachable!(), // return itr_err_fmt!(InstParamsErr, "local operand {} not find", a)
    }?;
    Ok(())
}


fn local_logic(mark: u8, locals: &mut Stack, value: &mut Value) -> VmrtErr {
    let opt = mark >> 5; // 0b00000111
    let idx = mark & 0b00011111; // max=32
    let basev = locals.edit(idx)?;
    match opt {
        0 => locop_btw(value, basev, lgc_and),
        1 => locop_btw(value, basev, lgc_or),
        2 => locop_btw(value, basev, lgc_equal),
        3 => locop_btw(value, basev, lgc_not_equal),
        4 => locop_btw(value, basev, lgc_less),
        5 => locop_btw(value, basev, lgc_less_equal),
        6 => locop_btw(value, basev, lgc_greater),
        7 => locop_btw(value, basev, lgc_greater_equal),
        _ => unreachable!(), // return itr_err_fmt!(InstParamsErr, "local operand {} not find", a)
    }?;
    Ok(())
}


fn unpack_list(mut i: u8, locals: &mut Stack, list: &VecDeque<Value>) -> VmrtErr {
    let start = i as usize;
    if locals.len() < start + list.len() {
        return itr_err_code!(OutOfLocal)
    }
    // replace
    for item in list.iter() {
        *locals.edit(i)? = item.clone();
        i += 1;
    }
    Ok(())
}


fn debug_print_value(_ctx: &ContractAddress, _cur: &ContractAddress 
, _mode: ExecMode, _depth: isize, _val: Value) {
    debug_println!("{}-{} {} {:?} => {:?}", 
        _ctx.prefix(7), _cur.prefix(7), _depth, _mode, _val);
}

#[inline(always)]
fn bytes_len(v: &Value) -> usize {
    match v {
        Value::Bytes(b) => b.len(),
        _ => 0,
    }
}

#[allow(unused)]
fn debug_print_stack(ops: &Stack, lcs: &Stack, pc: &usize, inst: Bytecode) {
    println!("operds({})={}\nlocals({})={}\n-------- pc = {}, nbt = {:?}", 
    ops.len(), &ops.print_stack(), lcs.len(), &lcs.print_stack(), 
    *pc, inst);

}




#[cfg(test)]
mod bounds_tests {
    use super::*;
    use crate::machine::VmHost;
    use crate::rt::{ExecMode, GasExtra, GasTable, ItrErr, ItrErrCode, SpaceCap, VmrtErr, VmrtRes};
    use crate::space::{CtcKVMap, GKVMap, Heap, Stack};
    use crate::value::Value;
    use crate::ContractAddress;
    use sys::Ret;

    #[derive(Default)]
    struct DummyHost;

    impl VmHost for DummyHost {
        fn height(&mut self) -> u64 {
            1
        }

        fn ext_action_call(&mut self, _kid: u16, _body: Vec<u8>) -> Ret<(u32, Vec<u8>)> {
            Ok((0, vec![]))
        }

        fn log_push(&mut self, _cadr: &ContractAddress, _items: Vec<Value>) -> VmrtErr {
            Ok(())
        }

        fn srest(&mut self, _hei: u64, _cadr: &ContractAddress, _key: &Value) -> VmrtRes<Value> {
            itr_err_code!(ItrErrCode::StorageError)
        }

        fn sload(&mut self, _hei: u64, _cadr: &ContractAddress, _key: &Value) -> VmrtRes<Value> {
            itr_err_code!(ItrErrCode::StorageError)
        }

        fn sdel(&mut self, _cadr: &ContractAddress, _key: Value) -> VmrtErr {
            itr_err_code!(ItrErrCode::StorageError)
        }

        fn ssave(
            &mut self,
            _gst: &GasExtra,
            _hei: u64,
            _cadr: &ContractAddress,
            _key: Value,
            _val: Value,
        ) -> VmrtRes<i64> {
            itr_err_code!(ItrErrCode::StorageError)
        }

        fn srent(
            &mut self,
            _gst: &GasExtra,
            _hei: u64,
            _cadr: &ContractAddress,
            _key: Value,
            _period: Value,
        ) -> VmrtRes<i64> {
            itr_err_code!(ItrErrCode::StorageError)
        }
    }

    #[test]
    fn execute_code_rejects_truncated_params() {
        use crate::rt::Bytecode;

        let codes = vec![Bytecode::PU16 as u8]; // missing 2 bytes param

        let mut pc: usize = 0;
        let mut gas: i64 = 1000;
        let mut host = DummyHost::default();

        let mut operands = Stack::new(256);
        let mut locals = Stack::new(256);
        let mut heap = Heap::new(64);
        let mut globals = GKVMap::new(20);
        let mut memorys = CtcKVMap::new(12);

        let cadr = ContractAddress::default();

        let res = execute_code(
            &mut pc,
            &codes,
            ExecMode::Main,
            false,
            0,
            &mut gas,
            &GasTable::new(1),
            &GasExtra::new(1),
            &SpaceCap::new(1),
            &mut operands,
            &mut locals,
            &mut heap,
            &mut globals,
            &mut memorys,
            &mut host,
            &cadr,
            &cadr,
        );

        assert!(matches!(res, Err(ItrErr(ItrErrCode::CodeOverflow, _))));
    }
}
