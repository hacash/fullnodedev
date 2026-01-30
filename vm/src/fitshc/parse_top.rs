use sys::{Ret, errf};
use sys::*;
use crate::rt::{KwTy, AbstCall};
use crate::contract::{Abst};
use crate::Token::*;
use field::Address;
use super::state::ParseState;
use super::parse_deploy::{parse_deploy};
use super::parse_func::{parse_function};

pub fn parse_top_level(state: &mut ParseState) -> Ret<()> {
    // optional pragma
    if let Some(Keyword(KwTy::Use)) = state.current() {
        state.advance();
        if let Some(Identifier(p)) = state.current() {
            if p == "pragma" {
                state.advance();
                state.advance(); // version
            }
        }
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

fn parse_contract_body_item(state: &mut ParseState) -> Ret<()> {
    match state.current() {
        Some(Keyword(KwTy::Deploy)) => {
            let info = parse_deploy(state)?;
            state.deploy = Some(info);
        },
        Some(Keyword(KwTy::Library)) => {
            state.advance();
            let libs = parse_addr_list(state)?;
            for (_name, addr) in libs {
                state.contract = state.contract.clone().lib(addr);
            }
        },
        Some(Keyword(KwTy::Inherit)) => {
            state.advance();
            let inherits = parse_addr_list(state)?;
            for (_name, addr) in inherits {
                state.contract = state.contract.clone().inh(addr);
            }
        },
        Some(Keyword(KwTy::Abstract)) => {
            state.advance(); // consume abstract
            let (_func, _smap, name) = parse_function(state, false)?;
            if let Ok(aid) = AbstCall::from_name(&name) {
                state.contract = state.contract.clone().syst(Abst::new(aid));
            }
        },
        Some(Keyword(KwTy::Function)) => {
            // consume 'function' inside parse_function
            let (func, smap, name) = parse_function(state, true)?; 
            state.contract = state.contract.clone().func(func);
            state.source_maps.push((name, smap));
        },
        _ => {
            state.advance(); 
        }
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
            return errf!("Expected lib/inherit name");
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
             return errf!("Expected address but got {:?}", state.current()); 
        };
        
        list.push((name, addr));

        if let Some(Partition(',')) = state.current() {
            state.advance();
        }
    }
    Ok(list)
}
