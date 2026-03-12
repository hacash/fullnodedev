use super::compile_body::{CompiledCode, compile_body};
use super::parse_deploy::parse_deploy;
use super::parse_func::{parse_func_body_tokens, parse_func_sig, parse_function};
use super::state::ParseState;
use crate::contract::Abst;
use crate::rt::Token::{self, *};
use crate::rt::{AbstCall, KwTy, calc_func_sign};
use crate::value::ValueTy;
use field::Address;
use sys::*;
use sys::{Ret, errf};

pub fn parse_top_level(state: &mut ParseState) -> Ret<()> {
    // optional pragma: `use pragma 0.1.0` or `pragma 0.1.0`
    let mut saw_use = false;
    if let Some(Keyword(KwTy::Use)) = state.current() {
        saw_use = true;
        state.advance();
    }
    let is_pragma = matches!(state.current(), Some(Keyword(KwTy::Pragma)))
        || matches!(state.current(), Some(Identifier(p)) if p == "pragma");
    if is_pragma {
        state.advance();
        // consume version tokens like 0.1.0 or v0.1.0
        loop {
            match state.current() {
                Some(Integer(_)) | Some(Identifier(_)) | Some(Keyword(KwTy::Dot)) => {
                    state.advance()
                }
                _ => break,
            }
        }
    } else if saw_use {
        return errf!("expected pragma after 'use'");
    }

    // contract Name {
    if let Some(Keyword(KwTy::Contract)) = state.current() {
        state.advance();
        if let Some(Identifier(name)) = state.current() {
            state.contract_name = name.clone();
            state.advance();
        }
        state.eat_partition('{')?;

        loop {
            if let Some(Partition('}')) = state.current() {
                state.advance();
                break;
            }
            parse_contract_body_item(state)?;
        }
    } else {
        // Fallback for files without contract wrapper
        while state.idx < state.max {
            parse_contract_body_item(state)?;
        }
    }

    Ok(())
}

/// Represents a parsed top-level constant value
#[derive(Debug, Clone)]
pub enum ConstValue {
    /// Unsigned integer: u8, u16, u32, u64, u128, u256
    Uint(u128),
    /// Boolean: true or false
    Bool(bool),
    /// Bytes: hex string (0x...) or binary (0b...)
    Bytes(Vec<u8>),
    /// Address:Hacash address
    Address(field::Address),
    /// String literal
    String(Vec<u8>),
}

fn parse_const_value(state: &mut ParseState) -> Ret<ConstValue> {
    let token = state
        .current()
        .cloned()
        .ok_or_else(|| format!("Expected const value but got EOF"))?;
    state.advance();

    match token {
        Token::Integer(n) => Ok(ConstValue::Uint(n)),
        Token::Bytes(b) => Ok(ConstValue::Bytes(b)),
        Token::Address(addr) => Ok(ConstValue::Address(addr)),
        Token::Identifier(s) => {
            match s.as_str() {
                "true" => Ok(ConstValue::Bool(true)),
                "false" => Ok(ConstValue::Bool(false)),
                _ => {
                    // Try to parse as hex bytes
                    if s.starts_with("0x") {
                        let v = s[2..].to_string();
                        match hex::decode(v) {
                            Ok(d) => return Ok(ConstValue::Bytes(d)),
                            Err(_) => return errf!("invalid hex bytes: {}", s),
                        }
                    }
                    errf!("invalid const value: {}", s)
                }
            }
        }
        Token::Keyword(kw) => match kw {
            KwTy::True => Ok(ConstValue::Bool(true)),
            KwTy::False => Ok(ConstValue::Bool(false)),
            _ => errf!("invalid const value keyword: {:?}", kw),
        },
        _ => errf!("expected const value but got {:?}", token),
    }
}

fn parse_contract_body_item(state: &mut ParseState) -> Ret<()> {
    match state.current() {
        Some(Keyword(KwTy::Const)) => {
            // Parse top-level const: const NAME = VALUE
            state.advance(); // consume 'const'
            let name = if let Some(Identifier(n)) = state.current() {
                n.clone()
            } else {
                return errf!("expected const name after 'const'");
            };
            state.advance();

            // Expect '='
            if let Some(Keyword(KwTy::Assign)) = state.current() {
                state.advance();
            } else {
                return errf!("expected '=' after const name");
            }

            // Parse the const value
            let value = parse_const_value(state)?;

            // Convert to string representation for storage
            let value_str = match &value {
                ConstValue::Uint(n) => format!("uint:{}", n),
                ConstValue::Bool(b) => format!("bool:{}", b),
                ConstValue::Bytes(b) => format!("bytes:0x{}", hex::encode(b)),
                ConstValue::Address(a) => format!("address:{}", a.to_readable()),
                ConstValue::String(s) => format!("string:{}", String::from_utf8_lossy(s)),
            };

            if state.consts.iter().any(|(n, _)| n == &name) {
                return errf!("duplicate const '{}'", name);
            }
            state.consts.push((name, value_str));
        }
        Some(Keyword(KwTy::Deploy)) => {
            let info = parse_deploy(state)?;
            state.deploy = Some(info);
        }
        Some(Keyword(KwTy::Library)) => {
            state.advance();
            let libs = parse_addr_list(state)?;
            for (name, addr) in libs {
                if state.libs.len() >= u8::MAX as usize {
                    return errf!("too many contract libraries: max {}", u8::MAX);
                }
                if !state.library_addrs.insert(addr) {
                    return errf!("duplicate library address");
                }
                // libidx is 0-based order
                state.libs.push((name, addr));
                state.contract = state.contract.clone().lib(addr);
            }
        }
        Some(Keyword(KwTy::Inherit)) => {
            state.advance();
            let inherit_list = parse_addr_list(state)?;
            for (_name, addr) in inherit_list {
                if state.inherit_addrs.len() >= u8::MAX as usize {
                    return errf!("too many inherit contracts: max {}", u8::MAX);
                }
                if !state.inherit_addrs.insert(addr) {
                    return errf!("duplicate inherit address");
                }
                state.contract = state.contract.clone().inh(addr);
            }
        }
        Some(Keyword(KwTy::Abstract)) => {
            state.advance(); // consume abstract

            // Check for ircode/bytecode modifier
            let mut is_ircode = false;
            while let Some(tk) = state.current() {
                match tk {
                    Keyword(KwTy::IrCode) => {
                        is_ircode = true;
                        state.advance();
                    }
                    Keyword(KwTy::ByteCode) => {
                        state.advance();
                    }
                    _ => break,
                }
            }

            let (name, args, ret_ty) = parse_func_sig(state)?;
            // return type must be integer error code if declared
            if let Some(rty) = ret_ty {
                let ok = matches!(
                    rty,
                    ValueTy::U8 | ValueTy::U16 | ValueTy::U32 | ValueTy::U64 | ValueTy::U128
                );
                if !ok {
                    return errf!("abstract '{}' return type must be integer (error code)", name);
                }
            }
            // parse body for abstract code
            let body_tokens = parse_func_body_tokens(state)?;
            let aid = AbstCall::from_name(&name).map_err(|e| e.to_string())?;
            // validate param types
            let expect = aid.param_types();
            if expect.len() != args.len() {
                return errf!(
                    "abstract '{}' params length mismatch: expected {}, got {}",
                    name,
                    expect.len(),
                    args.len()
                );
            }
            for (i, (_, ty)) in args.iter().enumerate() {
                if *ty != expect[i] {
                    return errf!(
                        "abstract '{}' param {} type mismatch: expected {:?}, got {:?}",
                        name,
                        i,
                        expect[i],
                        ty
                    );
                }
            }

            // compile abstract body using shared compile function
            let (_irnodes, compiled, source_map) = compile_body(
                body_tokens,
                args.clone(),
                &state.libs,
                &state.consts,
                is_ircode,
            )?;

            let abst = match compiled {
                CompiledCode::IrCode(ircodes) => Abst::new(aid).ircode(ircodes)?,
                CompiledCode::Bytecode(bts) => Abst::new(aid).bytecode(bts)?,
            };
            if !state.abst_signs.insert(aid.uint()) {
                return errf!("duplicate abstract '{}'", name);
            }
            state.contract = state.contract.clone().syst(abst);
            state
                .source_maps
                .push((format!("abstract::{}", name), source_map));
        }
        Some(Keyword(KwTy::Function)) => {
            // consume 'function' inside parse_function
            let (func, smap, name) = parse_function(state, true)?;
            let sign = calc_func_sign(&name);
            if !state.userfunc_signs.insert(sign) {
                return errf!("duplicate function '{}' signature", name);
            }
            state.contract = state.contract.clone().func(func);
            state.source_maps.push((name, smap));
        }
        Some(token) => return errf!("unexpected token in contract body: {:?}", token),
        None => return errf!("unexpected EOF while parsing contract body"),
    }
    Ok(())
}

fn parse_addr_list(state: &mut ParseState) -> Ret<Vec<(String, Address)>> {
    state.eat_partition('[')?;
    let mut list = vec![];
    loop {
        if let Some(Partition(']')) = state.current() {
            state.advance();
            break;
        }
        // Name : Address
        let name = if let Some(Identifier(n)) = state.current() {
            n.clone()
        } else {
            return errf!("expected lib/inherit name");
        };
        state.advance();

        if let Some(Keyword(KwTy::Colon)) = state.current() {
            state.advance();
        } else {
            state.eat_partition(':')?;
        }

        let addr = if let Some(Identifier(a)) = state.current() {
            let adr = Address::from_readable(a).map_err(|e| e.to_string())?;
            state.advance();
            adr
        } else if let Some(Address(a)) = state.current() {
            let adr = a.clone();
            state.advance();
            adr
        } else {
            return errf!("expected address but got {:?}", state.current());
        };
        addr.must_contract()?;

        list.push((name, addr));

        if let Some(Partition(',')) = state.current() {
            state.advance();
        }
    }
    Ok(list)
}
