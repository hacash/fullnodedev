//! JumpOverflow reproduction test.
//! Extracts failing control flow from hacdtestnet ControlFlowTest contract,
//! compiles via lang/syntax interfaces, and verifies/executes to find the bug.

use vm::ir::convert_ir_to_bytecode;
use vm::lang::*;
use vm::rt::{BytecodePrint, verify_bytecodes};

/// Minimal Fitsh main-call script equivalent to:
///   function test_if_true(a: u64) -> u64 { if a > 0 { return 1 } return 0 }
fn failing_if_true_script() -> &'static str {
    r##"
        param { a }
        if a > 0 {
            return 1
        }
        return 0
    "##
}

/// Equivalent to test_if_else
fn failing_if_else_script() -> &'static str {
    r##"
        param { a }
        if a > 10 {
            return 2
        } else {
            return 1
        }
    "##
}

/// Equivalent to test_while_loop
fn failing_while_script() -> &'static str {
    r##"
        param { n }
        var sum = 0
        var i = 0
        while i < n {
            sum += i
            i += 1
        }
        return sum
    "##
}

fn compile_and_verify(script: &str, _name: &str) -> Result<Vec<u8>, String> {
    let ircodes = lang_to_ircode(script).map_err(|e| format!("lang_to_ircode: {}", e))?;
    let bytecodes =
        convert_ir_to_bytecode(&ircodes).map_err(|e| format!("convert_ir_to_bytecode: {:?}", e))?;
    verify_bytecodes(&bytecodes).map_err(|e| format!("verify_bytecodes: {:?}", e))?;
    Ok(bytecodes)
}

#[test]
fn jump_overflow_if_true() {
    let script = failing_if_true_script();
    match compile_and_verify(script, "if_true") {
        Ok(codes) => {
            println!(
                "[PASS] if_true compiled and verified, {} bytes",
                codes.len()
            );
            if let Ok(s) = codes.bytecode_print(true) {
                println!("bytecode:\n{}", s);
            }
        }
        Err(e) => {
            if e.contains("JumpOverflow") || e.contains("16") {
                panic!(
                    "JumpOverflow reproduced for if_true:\n{}\nScript:\n{}",
                    e, script
                );
            }
            panic!("Unexpected error for if_true: {}", e);
        }
    }
}

/// Raw if/else without end: verify must fail with JumpOverflow (fitsh compile error).
#[test]
fn jump_overflow_if_else() {
    let script = failing_if_else_script();
    let ircodes = lang_to_ircode(script).expect("lang_to_ircode");
    let bytecodes = convert_ir_to_bytecode(&ircodes).expect("convert_ir_to_bytecode");
    match verify_bytecodes(&bytecodes) {
        Ok(_) => panic!("Raw if/else without end must fail verify (JMPSL targets len)"),
        Err(e) => {
            assert!(
                format!("{:?}", e).contains("JumpOverflow"),
                "Expected JumpOverflow, got: {:?}",
                e
            );
        }
    }
}

/// Verification: JMPSL target = len (19), out of bounds. Appending END makes target valid.
#[test]
fn jump_overflow_verify_end_fix() {
    use vm::rt::Bytecode;
    let script = failing_if_else_script();
    let ircodes = lang_to_ircode(script).expect("lang_to_ircode");
    let mut bytecodes = convert_ir_to_bytecode(&ircodes).expect("convert_ir_to_bytecode");
    let len = bytecodes.len();
    // JMPSL at 14, offset 2: target = 14 + 3 + 2 = 19 (interpreter/verify formula)
    // valid range for len=19: 0..=18, so 19 -> JumpOverflow
    assert!(
        verify_bytecodes(&bytecodes).is_err(),
        "raw bytecode must fail (JMPSL targets len)"
    );
    // Append END so we have 20 bytes; position 19 = END, valid target
    bytecodes.push(Bytecode::END as u8);
    verify_bytecodes(&bytecodes).expect("bytecode with END appended must verify");
    assert_eq!(bytecodes.len(), len + 1, "END adds 1 byte");
}

#[test]
fn jump_overflow_while() {
    let script = failing_while_script();
    match compile_and_verify(script, "while") {
        Ok(codes) => {
            println!("[PASS] while compiled and verified, {} bytes", codes.len());
            if let Ok(s) = codes.bytecode_print(true) {
                println!("bytecode:\n{}", s);
            }
        }
        Err(e) => {
            if e.contains("JumpOverflow") || e.contains("16") {
                panic!(
                    "JumpOverflow reproduced for while:\n{}\nScript:\n{}",
                    e, script
                );
            }
            panic!("Unexpected error for while: {}", e);
        }
    }
}
