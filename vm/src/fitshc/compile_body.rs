use crate::IRNode;
use crate::ir::{IRNodeArray, IRNodeLeaf, convert_ir_to_bytecode, drop_irblock_wrap};
use crate::ir::{push_addr, push_bytes, push_num};
use crate::lang::Syntax;
use crate::rt::{Bytecode, SourceMap, Token, verify_bytecodes};
use crate::value::ValueTy;
use field::Address;
use sys::Ret;

/// Compiled code result - either IR codes or bytecodes
pub enum CompiledCode {
    IrCode(Vec<u8>),
    Bytecode(Vec<u8>),
}

/// Parse a constant value string into an IRNode
/// Format: "type:value" where type is uint, bool, bytes, address, or string
fn parse_const_value(value_str: &str) -> Ret<Box<dyn IRNode>> {
    if value_str.starts_with("uint:") {
        let num_str = &value_str[5..];
        let num: u128 = num_str
            .parse()
            .map_err(|_| format!("invalid uint constant: {}", value_str))?;
        return Ok(push_num(num));
    }
    if value_str.starts_with("bool:") {
        let bool_str = &value_str[5..];
        return Ok(match bool_str {
            "true" => Box::new(IRNodeLeaf {
                hrtv: true,
                inst: Bytecode::PTRUE,
                text: "true".to_string(),
            }),
            "false" => Box::new(IRNodeLeaf {
                hrtv: true,
                inst: Bytecode::PFALSE,
                text: "false".to_string(),
            }),
            _ => return Err(format!("invalid bool constant: {}", value_str).into()),
        });
    }
    if value_str.starts_with("bytes:0x") {
        let hex_str = &value_str["bytes:0x".len()..];
        let bytes = hex::decode(hex_str)
            .map_err(|_| format!("invalid hex bytes constant: {}", value_str))?;
        return push_bytes(&bytes);
    }
    if value_str.starts_with("address:") {
        let addr_str = &value_str[8..];
        let addr = Address::from_readable(addr_str)
            .map_err(|_| format!("invalid address constant: {}", value_str))?;
        return Ok(push_addr(addr));
    }
    if value_str.starts_with("string:") {
        let str_content = &value_str[7..];
        let bytes = str_content.as_bytes().to_vec();
        return push_bytes(&bytes);
    }
    if let Ok(num) = value_str.parse::<u128>() {
        return Ok(push_num(num));
    }
    Err(format!("unrecognized constant format: {}", value_str).into())
}

#[cfg(test)]
mod compile_body_tests {
    use super::*;
    use crate::lang::Tokenizer;

    #[test]
    fn parse_const_bytes_keeps_first_byte() {
        let parsed = parse_const_value("bytes:0x575459").unwrap();
        let expected = push_bytes(&vec![0x57, 0x54, 0x59]).unwrap();
        assert_eq!(parsed.serialize(), expected.serialize());
    }

    #[test]
    fn rejects_contract_lib_count_overflow() {
        let body_tokens = Tokenizer::new(b"return 1").parse().unwrap();
        let addr = Address::from_readable("emqjNS9PscqdBpMtnC3Jfuc4mvZUPYTPS").unwrap();
        let libs: Vec<_> = (0..=u8::MAX as usize)
            .map(|idx| (format!("L{}", idx), addr.clone()))
            .collect();
        let err = match compile_body(body_tokens, vec![], &libs, &[], true) {
            Ok(_) => panic!("compile_body should fail for overflowing lib count"),
            Err(err) => err,
        };
        assert!(err.to_string().contains("too many contract libs: max 255"));
    }
}

/// Compile function/abstract body tokens to IR or bytecode
pub fn compile_body(
    body_tokens: Vec<Token>,
    args: Vec<(String, ValueTy)>,
    libs: &[(String, Address)],
    consts: &[(String, String)],
    is_ircode: bool,
) -> Ret<(IRNodeArray, CompiledCode, SourceMap)> {
    let mut syntax = Syntax::new(body_tokens);

    if !libs.is_empty() {
        if libs.len() > u8::MAX as usize {
            return Err(format!("too many contract libs: max {}", u8::MAX).into());
        }
        let mut lib_entries = Vec::with_capacity(libs.len());
        for (idx, (name, addr)) in libs.iter().enumerate() {
            lib_entries.push((name.clone(), idx as u8, Some(addr.clone())));
        }
        syntax = syntax.with_libs(lib_entries);
    }

    if !consts.is_empty() {
        let mut const_nodes = Vec::with_capacity(consts.len());
        for (name, value_str) in consts {
            const_nodes.push((name.clone(), parse_const_value(value_str)?));
        }
        syntax = syntax.with_consts(const_nodes);
    }

    syntax = syntax.with_params(args).with_ircode(is_ircode);

    let (irnodes, source_map) = syntax.parse()?;

    let compiled = if is_ircode {
        let ircodes = drop_irblock_wrap(irnodes.serialize())?;
        let codes = convert_ir_to_bytecode(&ircodes).map_err(|e| e.to_string())?;
        verify_bytecodes(&codes).map_err(|e| e.to_string())?;
        CompiledCode::IrCode(ircodes)
    } else {
        let bts = irnodes.codegen().map_err(|e| e.to_string())?;
        verify_bytecodes(&bts).map_err(|e| e.to_string())?;
        CompiledCode::Bytecode(bts)
    };

    Ok((irnodes, compiled, source_map))
}
