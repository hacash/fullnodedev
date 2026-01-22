// use sys::*;
// use vm::IRNode;
// use vm::rt::BytecodePrint;
// use vm::ir::IRCodePrint;
// use vm::lang::{Tokenizer, Syntax};

use vm::lang::*;
use vm::rt::*;
use vm::ir::*;



#[test]
fn t1(){
    
    // lang_to_bytecode("return 0").unwrap();

    let payable_hac_fitsh = r##"
        // var addr = 1
        self.deposit(1)
        end
    "##;

    let ircodes = lang_to_ircode(&payable_hac_fitsh).unwrap();

    println!("\n{}\n", ircodes.bytecode_print(false).unwrap());

    let bytecodes = convert_ir_to_bytecode(&ircodes).unwrap();

    println!("\n{}\n", bytecodes.bytecode_print(false).unwrap());

        
}

#[test]
fn let_slot_and_cache_print() {
    let script = r##"
        let $0 = 1
        let foo $1 = $0
        let bar = foo
        print bar
        print bar
    "##;
    let ircodes = lang_to_ircode(script).unwrap();
    let printed = ircodes.ircode_print(true).unwrap();
    assert!(printed.matches("print($0)").count() >= 2);
}

#[test]
fn calllib_callself_print() {
    let script = r##"
        calllib 2::abcdef01()
        callself 11223344()
        callstatic 3::deadbeef()
    "##;
    let ircodes = lang_to_ircode(script).unwrap();
    let printed = ircodes.ircode_print(true).unwrap();
    assert!(printed.contains("calllib 2::abcdef01("));
    assert!(printed.contains("callself 00ab4130("));
    assert!(printed.contains("callstatic 3::deadbeef("));
}

#[test]
fn let_var_interleave_print() {
    let script = r##"
        var x $0 = 10
        let aux $1 = x
        var y = aux
        let cache = y
        print x
        print cache
        print $1
    "##;
    let ircodes = lang_to_ircode(script).unwrap();
    let printed = ircodes.ircode_print(true).unwrap();
    assert!(printed.contains("$0 = 10"));
    assert!(printed.contains("$1 = $0"));
    assert!(printed.matches("print($0)").count() >= 1);
    assert!(printed.matches("print($1)").count() >= 2);
}

#[test]
fn print_decomp_let_alias_clones_expression() {
    let script = r##"
        let base = {
            if true {
                { 1 }
            } else {
                { 2 }
            }
        }
        let alias = base
        print base
        print alias
    "##;
    let printed = lang_to_ircode(script).unwrap().ircode_print(true).unwrap();
    assert!(printed.matches("print(").count() >= 2);
    assert!(printed.matches("if 1 {").count() >= 2);
    assert!(printed.contains("} else {"));
}

#[test]
fn block_and_if_expression_use_expr_opcodes() {
    let script = r##"
        print {
            if false {
                1
            } else {
                2
            }
        }
        print if true { 3 } else { 4 }
    "##;
    let ircodes = lang_to_ircode(script).unwrap();
    assert!(ircodes.iter().any(|b| *b == Bytecode::IRBLOCKR as u8));
    assert!(ircodes.iter().any(|b| *b == Bytecode::IRIFR as u8));
}

#[test]
fn block_and_if_statement_use_stmt_opcodes() {
    let script = r##"
        if true {
            print 1
        } else {
            print 2
        }
        {
            print 3
        }
    "##;
    let ircodes = lang_to_ircode(script).unwrap();
    assert!(ircodes.iter().any(|b| *b == Bytecode::IRIF as u8));
    assert!(ircodes.iter().any(|b| *b == Bytecode::IRBLOCK as u8));
    assert!(!ircodes.iter().any(|b| *b == Bytecode::IRBLOCKR as u8));
    assert!(!ircodes.iter().any(|b| *b == Bytecode::IRIFR as u8));
}

#[test]
fn nested_expression_contexts_emit_expr_opcodes() {
    let script = r##"
        print {
            if true {
                let inner = if false {
                    { if false { 10 } else { 11 } }
                } else {
                    { { 12 } }
                }
                inner
            } else {
                {
                    let deep = { if true { { 13 } } else { { 14 } } }
                    deep
                }
            }
        }
        print { { if true { { 15 } } else { { 16 } } } }
        print if false { { 17 } } else { { 18 } }
        print if true { { 19 } } else { { 20 } }
    "##;
    let ircodes = lang_to_ircode(script).unwrap();
    let blockr = ircodes.iter().filter(|b| **b == Bytecode::IRBLOCKR as u8).count();
    let ifr = ircodes.iter().filter(|b| **b == Bytecode::IRIFR as u8).count();
    assert!(blockr >= 5);
    assert!(ifr >= 4);
}

#[test]
fn var_rhs_block_expression_emits_expr_opcodes() {
    let script = r##"
        var holder = {
            if true {
                let inner = if false {
                    {
                        if true { 1 } else { 2 }
                    }
                } else {
                    { 3 }
                }
                inner
            } else {
                { 4 }
            }
        }
        var stmt = {
            print 5
            0
        }
    "##;
    let ircodes = lang_to_ircode(script).unwrap();
    assert!(ircodes.iter().any(|b| *b == Bytecode::IRBLOCKR as u8));
    assert!(ircodes.iter().any(|b| *b == Bytecode::IRIFR as u8));
}
