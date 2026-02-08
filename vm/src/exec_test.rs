//! Standalone VM execution helper for integration tests.
//! Compiles lang script, pushes params, executes, returns result or error.

use basis::component::Env;
use basis::interface::{Context, TransactionRead};
use field::{Address, Amount, Hash};
use protocol::context::ContextInst;
use protocol::state::EmptyLogs;
use std::collections::HashMap;
use sys::Ret;

use crate::lang::lang_to_bytecode;
use crate::machine::CtxHost;
use crate::rt::*;
use crate::rt::Bytecode::*;
use crate::space::{CtcKVMap, GKVMap, Heap, Stack};
use crate::value::ValueTy;
use crate::value::ValueTy::*;

#[derive(Default, Clone, Debug)]
struct DummyTx;

impl field::Serialize for DummyTx {
    fn size(&self) -> usize { 0 }
    fn serialize(&self) -> Vec<u8> { vec![] }
}

impl basis::interface::TxExec for DummyTx {}

impl TransactionRead for DummyTx {
    fn ty(&self) -> u8 { 3 }
    fn hash(&self) -> Hash { Hash::default() }
    fn hash_with_fee(&self) -> Hash { Hash::default() }
    fn main(&self) -> Address { Address::default() }
    fn addrs(&self) -> Vec<Address> { vec![Address::default()] }
    fn fee(&self) -> &Amount { Amount::zero_ref() }
    fn fee_purity(&self) -> u64 { 1 }
    fn fee_extend(&self) -> Ret<u8> { Ok(1) }
}

#[derive(Default)]
struct StateMem {
    mem: HashMap<Vec<u8>, Vec<u8>>,
}

impl basis::interface::State for StateMem {
    fn get(&self, k: Vec<u8>) -> Option<Vec<u8>> {
        self.mem.get(&k).cloned()
    }
    fn set(&mut self, k: Vec<u8>, v: Vec<u8>) {
        self.mem.insert(k, v);
    }
    fn del(&mut self, k: Vec<u8>) {
        self.mem.remove(&k);
    }
}

/// Build bytecode to push params onto stack (same format as sandbox_call).
/// Single param: raw value. Multi param: PACKLIST.
pub fn build_push_params(params: &str) -> Ret<Vec<u8>> {
    let mut codes = vec![];
    macro_rules! push { ( $( $a: expr ),+) => { $( codes.push($a as u8) );+ } }

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
        _ => return errf!("param number is too much"),
    }
    Ok(codes)
}

/// Execute lang script with params string. Returns Ok(Value) on success, Err(ItrErr) on VM error.
/// Use for standalone tests to verify VM behavior (e.g. CastFail, CompoOpNotMatch).
pub fn execute_lang_with_params(lang_script: &str, params: &str) -> VmrtRes<crate::value::Value> {
    let push_codes = build_push_params(params).map_err(|e| crate::rt::ItrErr::new(crate::rt::ItrErrCode::InstParamsErr, &e))?;
    let body_codes = lang_to_bytecode(lang_script).map_err(|e| crate::rt::ItrErr::new(crate::rt::ItrErrCode::InstParamsErr, &e))?;
    let mut codes = push_codes;
    codes.extend_from_slice(&body_codes);

    let mut pc = 0usize;
    let mut gas: i64 = 65535;
    let cadr = crate::ContractAddress::default();

    let tx = DummyTx::default();
    let mut env = Env::default();
    env.block.height = 1;
    let mut ctx = ContextInst::new(
        env,
        Box::new(StateMem::default()),
        Box::new(EmptyLogs {}),
        &tx,
    );
    let ctx: &mut dyn Context = &mut ctx;

    let mut ops = Stack::new(256);
    let mut heap = Heap::new(64);
    let mut host = CtxHost::new(ctx);

    crate::interpreter::execute_code(
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
        .ok_or_else(|| crate::rt::ItrErr::new(crate::rt::ItrErrCode::StackError, "no return value"))
}
