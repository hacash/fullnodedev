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
    assert!(printed.contains("let $0 ="));
    assert!(printed.contains("let $1 ="));
    assert!(printed.contains("let $2 ="));
    assert!(printed.matches("$2").count() >= 2);
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
    assert!(printed.contains("let $1 ="));
    assert!(printed.contains("var"));
    assert!(printed.matches("$1").count() >= 2);
    assert!(printed.contains("$0"));
}
