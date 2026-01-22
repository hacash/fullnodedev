#![allow(dead_code)]

use vm::IRNode;
use vm::ir::{convert_ir_to_bytecode, parse_ir_block, IRCodePrint};
use vm::lang::{lang_to_ircode, lang_to_irnode};
use vm::rt::Bytecode;

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
    let _ = lang_to_irnode(script).unwrap();
    let ircodes = lang_to_ircode(script).unwrap();
    ensure_ir_roundtrip(&ircodes);

    let decompiled = ircodes.ircode_print(true).unwrap();
    if let Ok(ircodes_roundtrip) = lang_to_ircode(&decompiled) {
        let ircodes_roundtrip = unwrap_root_block(ircodes_roundtrip);
        ensure_ir_roundtrip(&ircodes_roundtrip);
        assert_eq!(ircodes, ircodes_roundtrip);
        let bytecodes = convert_ir_to_bytecode(&ircodes).unwrap();
        let bytecodes_roundtrip = convert_ir_to_bytecode(&ircodes_roundtrip).unwrap();
        assert_eq!(bytecodes, bytecodes_roundtrip);
    } else {
        eprintln!("warning: Fitsh roundtrip parse failed for script:\n{}\n", decompiled);
    }
    ircodes
}

pub fn compile_fitsh_bytecode(script: &str) -> Vec<u8> {
    let ircodes = checked_compile_fitsh_to_ir(script);
    convert_ir_to_bytecode(&ircodes).unwrap()
}
