use sys::{Ret, errf};
use sys::*;
use crate::rt::{Token, KwTy, Bytecode};
use crate::value::ValueTy;
use crate::Token::*;
use crate::contract::Func;
use crate::lang::Syntax;
use crate::rt::SourceMap;
use crate::IRNode;
use super::state::ParseState;

pub fn parse_function(state: &mut ParseState, consume_kw: bool) -> Ret<(Func, SourceMap, String)> {
    // function public/private/ircode Name(...) -> Ret { ... }
    if consume_kw {
        state.advance(); // function
    }

    let mut is_public = false;
    let mut is_ircode = false;
    
    // Modifiers
    while let Some(tk) = state.current() {
        match tk {
            Keyword(KwTy::Public) => {
                is_public = true;
                state.advance();
            },
            Keyword(KwTy::Private) => {
                state.advance();
            },
            Keyword(KwTy::IrCode) => {
                is_ircode = true;
                state.advance();
            },
            Keyword(KwTy::ByteCode) => {
                state.advance();
            },
            _ => break,
        }
    }

    let (name, args, ret_ty) = parse_func_sig(state)?;
    let body_tokens = parse_func_body_tokens(state)?;

    // Setup Func
    let mut func = Func::new(&name);
    if is_public { func = func.public(); }
    
    let arg_types: Vec<ValueTy> = args.iter().map(|(_, t)| *t).collect();
    func = func.types(ret_ty, arg_types);
    
    // Compile body
    let mut syntax = Syntax::new(body_tokens);
    // Inject params
    syntax = syntax.with_params(args);

    let (irnodes, source_map) = syntax.parse()?; 
    
    if is_ircode {
         func = func.irnode(irnodes)?;
    } else {
         // Default to bytecode
         let mut bts = irnodes.codegen().map_err(|e| e.to_string())?;
         // Append END if not present? 
         bts.push(Bytecode::END as u8);
         func = func.bytecode(bts)?;
    }

    Ok((func, source_map, name))
}


pub fn parse_func_sig(state: &mut ParseState) -> Ret<(String, Vec<(String, ValueTy)>, Option<ValueTy>)> {
    // Name(args) -> Ret
    let name = if let Some(Identifier(n)) = state.current() {
        let n = n.clone();
        state.advance();
        n
    } else {
        return errf!("expected function name but got {:?}", state.current())
    };
    
    state.eat_partition('(')?;
    
    let mut args = Vec::new();
    loop {
        if let Some(Partition(')')) = state.current() {
            state.advance();
            break;
        }
        if state.idx >= state.max { return errf!("args not closed") }

        // arg: type
        let arg_name = if let Some(Identifier(n)) = state.current() {
             let n = n.clone();
             state.advance();
             n
        } else {
             return errf!("expected arg name")
        };

        // :
        if let Some(Keyword(KwTy::Colon)) = state.current() {
            state.advance();
        }
        
        // type
        let rtype = parse_type(state);
        let aty = match rtype {
            Some(t) => t,
            None => return errf!("unknown type")
        };
        args.push((arg_name, aty));
        
        // comma
        if let Some(Partition(',')) = state.current() {
             state.advance();
        }
    }
    
    // -> Ret
    let mut ret_ty = None;
    if let Some(Keyword(KwTy::Arrow)) = state.current() {
        state.advance();
        
        // ( type )
        if let Some(Partition('(')) = state.current() {
             state.advance();
        }

        if state.idx >= state.max { return errf!("expected return type") }
        let rtype = parse_type(state);
        
        ret_ty = match rtype {
            Some(t) => Some(t),
            None => return errf!("unknown return type")
        };

        if let Some(Partition(')')) = state.current() {
             state.advance();
        }
    }
    
    Ok((name, args, ret_ty))
}

pub fn parse_type(state: &mut ParseState) -> Option<ValueTy> {
    if state.idx >= state.max { return None }
    let tk = &state.tokens[state.idx];
    let ty = if let Keyword(k) = tk {
        match k {
            KwTy::U8 => Some(ValueTy::U8),
            KwTy::U16 => Some(ValueTy::U16),
            KwTy::U32 => Some(ValueTy::U32),
            KwTy::U64 => Some(ValueTy::U64),
            KwTy::U128 => Some(ValueTy::U128),
            KwTy::Address => Some(ValueTy::Address),
            KwTy::Bytes => Some(ValueTy::Bytes),
            KwTy::Bool => Some(ValueTy::Bool),
             _ => None 
        }
    } else if let Identifier(tn) = tk {
        match tn.as_str() {
            "u8" => Some(ValueTy::U8),
             "u16" => Some(ValueTy::U16),
            "u32" => Some(ValueTy::U32),
            "u64" => Some(ValueTy::U64),
            "u128" => Some(ValueTy::U128),
            "address" => Some(ValueTy::Address),
            "bytes" => Some(ValueTy::Bytes),
            "bool" => Some(ValueTy::Bool),
            _ => None
        }
    } else { None };
    
    if ty.is_some() {
        state.advance();
    }
    ty
}

fn parse_func_body_tokens(state: &mut ParseState) -> Ret<Vec<Token>> {
    state.eat_partition('{')?;
    let mut inner = Vec::new();
    let mut depth = 1;
    while state.idx < state.max {
        let t = &state.tokens[state.idx];
        if let Partition('{') = t { depth += 1; }
        if let Partition('}') = t { depth -= 1; }
        if depth == 0 {
            state.advance(); // consume closing }
            return Ok(inner);
        }
        inner.push(t.clone());
        state.advance();
    }
    errf!("bracket mismatch")
}
