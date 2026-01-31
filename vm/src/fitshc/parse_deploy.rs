use sys::{Ret, errf};
use sys::*;
use crate::rt::*;
use crate::Token::*;
use field::{Amount, Uint4, BytesW1};
use super::state::ParseState;

#[derive(Default, Debug, Clone)]
pub struct DeployInfo {
    pub protocol_cost: Option<Amount>,
    pub nonce: Option<Uint4>,
    pub construct_argv: Option<BytesW1>,
     pub matches: bool,
}


pub fn parse_deploy(state: &mut ParseState) -> Ret<DeployInfo> {
    state.advance(); // consume deploy
    state.eat_partition('{')?;
    
    let mut info = DeployInfo::default();
    info.matches = true;

    loop {
        if let Some(Partition('}')) = state.current() {
            state.advance();
            break;
        }

        let key = if let Some(Identifier(n)) = state.current() {
            let n = n.clone();
            state.advance();
            n
        } else {
             return errf!("expected deploy key")
        };
        
        // let mut key_colon = false;
        if let Some(Keyword(KwTy::Colon)) = state.current() {
            // key_colon = true;
            state.advance();
        } else if let Some(Partition(':')) = state.current() {
            // key_colon = true;
            state.advance();
        } 
        
        if key == "protocol_cost" {
             // value
             // If tokenizer splits "1:248", we might see "1" ":" "248" or string "1:248" depending on tokenizer.
             // Tokenizer treats ':' as symbol. So "1:248" becomes Int(1), Sym(:), Int(248).
             // But if user quoted it '"1:248"', it becomes Bytes(utf8).
             if let Some(Bytes(v)) = state.current() {
                 let s = String::from_utf8_lossy(v);
                 let amt = Amount::from(s.as_ref()).map_err(|e| e.to_string())?;
                 info.protocol_cost = Some(amt);
                 state.advance();
             } else if let Some(Identifier(v)) = state.current() {
                 let amt = Amount::from(v).map_err(|e| e.to_string())?;
                 info.protocol_cost = Some(amt);
                 state.advance();
             } else if let Some(Integer(v)) = state.current() {
                // support 1:248 without quotes
                let mut val = v.to_string();
                let mut consumed = false;
                if state.idx + 2 < state.max {
                    let has_colon = matches!(
                        state.tokens.get(state.idx + 1),
                        Some(Keyword(KwTy::Colon)) | Some(Partition(':'))
                    );
                    if has_colon {
                        if let Some(Integer(v2)) = state.tokens.get(state.idx + 2) {
                            val = format!("{}:{}", v, v2);
                            state.advance(); // first integer
                            state.advance(); // colon
                            state.advance(); // second integer
                            consumed = true;
                        }
                    }
                }
                let amt = Amount::from(val.as_str()).map_err(|e| e.to_string())?;
                info.protocol_cost = Some(amt);
                if !consumed {
                    state.advance();
                }
            } else {
                 return errf!("expected amount at protocol_cost")
             }
        } else if key == "nonce" {
             if let Some(Integer(v)) = state.current() {
                 info.nonce = Some(Uint4::from(*v as u32));
                 state.advance();
             } else if let Some(Identifier(v)) = state.current() {
                 let n = v.parse::<u32>().map_err(|e| e.to_string())?;
                 info.nonce = Some(Uint4::from(n));
                 state.advance();
             } else {
                 return errf!("expected nonce integer")
             }
        } else if key == "construct_argv" {
             // Support hex string
             if let Some(Bytes(v)) = state.current() {
                 let s = String::from_utf8_lossy(v);
                 let s = s.as_ref();
                 let bts = if let Some(hexstr) = s.strip_prefix("0x") {
                     hex::decode(hexstr).map_err(|e| e.to_string())?
                 } else {
                     s.as_bytes().to_vec()
                 };
                 info.construct_argv = Some(BytesW1::from(bts).unwrap());
                 state.advance();
             } else if let Some(Identifier(v)) = state.current() {
                 let bts = if let Some(hexstr) = v.strip_prefix("0x") {
                     hex::decode(hexstr).map_err(|e| e.to_string())?
                 } else {
                     v.as_bytes().to_vec()
                 };
                 info.construct_argv = Some(BytesW1::from(bts).unwrap());
                 state.advance();
             } else {
                 return errf!("expected argv value")
             }
        } else {
            // ignore or error?
            state.advance(); 
        }

        // comma?
        if let Some(Partition(',')) = state.current() {
            state.advance();
        }
    }

    Ok(info)
}
