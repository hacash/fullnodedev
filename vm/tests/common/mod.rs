#![allow(dead_code)]

use basis::component::Env;
use basis::interface::Context;
use testkit::sim::context::make_ctx_with_state;
use testkit::sim::state::FlatMemState;
use testkit::sim::tx::DummyTx;
use vm::IRNode;
use vm::ContractAddress;
use vm::interpreter::execute_code;
use vm::ir::{convert_ir_to_bytecode, parse_ir_block};
use vm::lang::*;
use vm::machine::CtxHost;
use vm::rt::Bytecode::*;
use vm::rt::{Bytecode, BytecodePrint, ExecMode, GasExtra, GasTable, ItrErr, ItrErrCode, SpaceCap, VmrtRes};
use vm::space::{CtcKVMap, GKVMap, Heap, Stack};
use vm::value::{Value, ValueTy};
use vm::value::ValueTy::*;

fn ensure_ir_roundtrip(bytes: &[u8]) {
    let mut idx = 0;
    let parsed = parse_ir_block(bytes, &mut idx).unwrap();
    assert_eq!(idx, bytes.len());
    let serialized = parsed.serialize();
    assert!(serialized.len() >= 3);
    assert_eq!(&serialized[3..], bytes);
}

fn unwrap_root_block(bytes: Vec<u8>) -> Vec<u8> {
    let mut idx = 0;
    if let Ok(parsed) = parse_ir_block(&bytes, &mut idx) {
        if idx == bytes.len() && parsed.len() == 1 {
            let first = &parsed[0];
            let inst = first.bytecode();
            if inst == Bytecode::IRBLOCK as u8 || inst == Bytecode::IRBLOCKR as u8 {
                let mut serialized = first.serialize();
                if serialized.len() >= 3 {
                    return serialized.split_off(3);
                }
            }
        }
    }
    bytes
}

pub fn checked_compile_fitsh_to_ir(script: &str) -> Vec<u8> {
    // println!("--------------checked_compile_fitsh_to_ir--------++++++++++++++++\n{}", script);
    let (ircd1, smap) = lang_to_ircode_with_sourcemap(script).unwrap();
    let lang1 = format_ircode_to_lang(&ircd1, Some(&smap)).unwrap();
    let res1 = lang_to_ircode_with_sourcemap(&lang1);
    if let Err(e) = &res1 {
        println!("Original Script:\n{}", script);
        println!("Reconstructed Script:\n{}", lang1);
        panic!("Fitsh roundtrip failed, {}", e);
    };
    let (ircd2, _) = res1.unwrap();
    if ircd1 != ircd2 {
        println!("Original Script:\n{}", script);
        println!("Reconstructed Script:\n{}", lang1);
        println!("-- {:?}", ircd1);
        println!("-- {:?}", ircd2);
        panic!("Fitsh roundtrip IR mismatch");
    }
    assert_eq!(ircd1, ircd2);

    // check other
    let _ = lang_to_irnode(script).unwrap();
    let ircodes = lang_to_ircode(script).unwrap();
    ensure_ir_roundtrip(&ircodes);

    let decompiled = ircodes.bytecode_print(true).unwrap();
    if let Ok(ircodes_roundtrip) = lang_to_ircode(&decompiled) {
        let ircodes_roundtrip = unwrap_root_block(ircodes_roundtrip);
        ensure_ir_roundtrip(&ircodes_roundtrip);
        assert_eq!(ircodes, ircodes_roundtrip);
        let bytecodes = convert_ir_to_bytecode(&ircodes).unwrap();
        let bytecodes_roundtrip = convert_ir_to_bytecode(&ircodes_roundtrip).unwrap();
        assert_eq!(bytecodes, bytecodes_roundtrip);
    } else {
        eprintln!(
            "warning: Fitsh roundtrip parse failed for script:\n{}\n",
            decompiled
        );
    }
    ircodes
}

pub fn compile_fitsh_bytecode(script: &str) -> Vec<u8> {
    let ircodes = checked_compile_fitsh_to_ir(script);
    convert_ir_to_bytecode(&ircodes).unwrap()
}

/// Build bytecode to push params onto stack (same format as sandbox_call).
/// Single param: raw value. Multi param: PACKLIST.
pub fn build_push_params(params: &str) -> sys::Ret<Vec<u8>> {
    let mut codes = vec![];
    macro_rules! push {
        ( $( $a: expr ),+ ) => {
            {
                $( codes.push($a as u8); )+
            }
        }
    }

    let pms: Vec<_> = params.split(',').collect();
    let mut pms_count = 0usize;
    for part in &pms {
        let s: Vec<_> = part.split(':').collect();
        let v = s.get(0).copied().unwrap_or("");
        let t = s.get(1).copied().unwrap_or("nil");
        let ty = ValueTy::from_name(t);
        let Ok(ty) = ty else {
            continue;
        };
        match ty {
            Nil => push!(PNIL),
            Bool => push!(if v == "true" { PTRUE } else { PFALSE }),
            U8 => {
                if let Ok(n) = v.parse::<u8>() {
                    push!(PU8, n);
                }
            }
            U16 => {
                if let Ok(n) = v.parse::<u16>() {
                    push!(PU16);
                    codes.extend_from_slice(&n.to_be_bytes());
                }
            }
            U32 => {
                if let Ok(n) = v.parse::<u32>() {
                    push!(PBUF, 4);
                    codes.extend_from_slice(&n.to_be_bytes());
                    push!(CU32);
                }
            }
            U64 => {
                if let Ok(n) = v.parse::<u64>() {
                    push!(PBUF, 8);
                    codes.extend_from_slice(&n.to_be_bytes());
                    push!(CU64);
                }
            }
            U128 => {
                if let Ok(n) = v.parse::<u128>() {
                    push!(PBUF, 16);
                    codes.extend_from_slice(&n.to_be_bytes());
                    push!(CU128);
                }
            }
            Address => {
                if let Ok(adr) = field::Address::from_readable(v) {
                    push!(PBUF, field::Address::SIZE);
                    codes.extend_from_slice(&adr.into_vec());
                    push!(CTO, ty);
                }
            }
            Bytes => {
                if let Ok(bts) = hex::decode(v) {
                    push!(PBUF, bts.len());
                    codes.extend_from_slice(&bts);
                }
            }
            _ => {}
        }
        pms_count += 1;
    }

    match pms_count {
        0 => push!(PNIL),
        1 => {}
        2..=254 => {
            push!(PU8, pms_count as u8);
            push!(PACKLIST);
        }
        _ => return Err("param number is too much".to_string()),
    }
    Ok(codes)
}

/// Execute lang script with params string. Returns Ok(Value) on success, Err(ItrErr) on VM error.
/// Use for standalone tests to verify VM behavior (e.g. CastFail, CompoOpNotMatch).
pub fn execute_lang_with_params(lang_script: &str, params: &str) -> VmrtRes<Value> {
    let push_codes =
        build_push_params(params).map_err(|e| ItrErr::new(ItrErrCode::InstParamsErr, &e))?;
    let body_codes = lang_to_bytecode(lang_script)
        .map_err(|e| ItrErr::new(ItrErrCode::InstParamsErr, &e))?;
    let mut codes = push_codes;
    codes.extend_from_slice(&body_codes);

    let mut pc = 0usize;
    let mut gas: i64 = 65535;
    let cadr = ContractAddress::default();

    let tx = DummyTx::default();
    let mut env = Env::default();
    env.block.height = 1;
    let mut ctx = make_ctx_with_state(env, Box::new(FlatMemState::default()), &tx);
    let ctx: &mut dyn Context = &mut ctx;

    let mut ops = Stack::new(256);
    let mut heap = Heap::new(64);
    let mut host = CtxHost::new(ctx);

    execute_code(
        &mut pc,
        &codes,
        ExecMode::Main,
        false,
        0,
        &mut gas,
        &GasTable::new(1),
        &GasExtra::new(1),
        &SpaceCap::new(1),
        &mut ops,
        &mut Stack::new(256),
        &mut heap,
        &mut GKVMap::new(20),
        &mut CtcKVMap::new(12),
        &mut host,
        &cadr,
        &cadr,
    )?;

    let released = ops.release();
    released
        .into_iter()
        .last()
        .ok_or_else(|| ItrErr::new(ItrErrCode::StackError, "no return value"))
}
