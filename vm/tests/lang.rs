// use sys::*;
// use vm::IRNode;
// use vm::rt::BytecodePrint;
// use vm::ir::IRCodePrint;
// use vm::lang::{Tokenizer, Syntax};

use vm::IRNode;
use vm::PrintOption;
use vm::ir::*;
use vm::lang::*;
use vm::rt::*;

#[test]
fn t1() {
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
        var x $0 = 1
        let foo = $0
        let bar = foo
        print bar
        print bar
    "##;
    let ircodes = lang_to_ircode(script).unwrap();
    let printed = ircode_to_lang(&ircodes).unwrap();
    assert!(printed.matches("print $0").count() >= 2);
}

#[test]
fn calllib_callinr_print() {
    let script = r##"
        calllib 2::abcdef01()
        callinr 11223344()
        callstatic 3::deadbeef()
    "##;
    let ircodes = lang_to_ircode(script).unwrap();
    let printed = ircode_to_lang(&ircodes).unwrap();
    assert!(printed.contains("calllib 2:abcdef01("));
    assert!(printed.contains("callinr 00ab4130("));
    assert!(printed.contains("callstatic 3::deadbeef("));
}

#[test]
fn call_keyword_print() {
    let script = r##"
        call 1::abcdef01()
    "##;
    let ircodes = lang_to_ircode(script).unwrap();
    let printed = ircode_to_lang(&ircodes).unwrap();
    assert!(printed.contains("call 1::abcdef01("));
}

#[test]
fn var_put_print_roundtrip() {
    let script = r##"
        var total $0 = 1
        total = total + 1
        var other $1 = total
        other = $1
    "##;
    let (block, source_map) = lang_to_irnode_with_sourcemap(script).unwrap();
    let opt = PrintOption::new("    ", 0, true).with_source_map(&source_map);
    let printed = block.print(&opt);
    assert!(printed.contains("var total $0 = 1"));
    assert!(printed.contains("total = "));
    assert!(printed.contains("var other $1 = total"));
    assert!(printed.contains("other = other"));
}

#[test]
fn var_cannot_rebind_param_slot() {
    let script = r##"
        param { addr, amt }
        var zhu $1 = amt
    "##;
    let err = lang_to_ircode(script).unwrap_err();
    assert!(err.contains("slot 1 already bound"));
}

#[test]
fn let_var_interleave_print() {
    let script = r##"
        var x $0 = 10
        let aux = x
        var y = aux
        let cache = y
        print x
        print cache
    "##;
    let ircodes = lang_to_ircode(script).unwrap();
    let printed = ircode_to_lang(&ircodes).unwrap();
    assert!(printed.contains("$0 = 10"));
    assert!(printed.contains("$1 = $0"));
    assert!(printed.matches("print $0").count() >= 1);
    assert!(printed.matches("print $1").count() >= 1);
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
    let ircode = lang_to_ircode(script).unwrap();
    let printed = ircode_to_lang(&ircode).unwrap();
    println!("{}", printed);
    assert!(printed.matches("print ").count() >= 2);
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
    let blockr = ircodes
        .iter()
        .filter(|b| **b == Bytecode::IRBLOCKR as u8)
        .count();
    let ifr = ircodes
        .iter()
        .filter(|b| **b == Bytecode::IRIFR as u8)
        .count();
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

fn check_fitsh_ir_roundtrip(script: &str, keywords: &[&str]) {
    let _ = lang_to_irnode(script).unwrap();
    let ircode_bytes = lang_to_ircode(script).unwrap();
    let printed = ircode_to_lang(&ircode_bytes).unwrap();
    for &kw in keywords {
        assert!(
            printed.contains(kw),
            "Fitsh decompile output missing '{}'\n{}",
            kw,
            printed
        );
    }
    let mut idx = 0;
    let _ = parse_ir_block(&ircode_bytes, &mut idx).unwrap();
    assert_eq!(idx, ircode_bytes.len());
}

#[test]
fn fitsh_ir_roundtrip_suite() {
    let scripts: [(&str, &[&str]); 3] = [
        (
            r##"
                lib HacSwap = 1: VFE6Zu4Wwee1vjEkQLxgVbv3c6Ju9iTaa
                param { amt }
                var counter = amt
                while counter > 0 {
                    counter -= 1
                }
                let sum = {
                    var builder = counter
                    builder += amt
                    builder
                }
                if sum > 0 {
                    calllib 1::abcdef01(sum)
                } else {
                    callstatic 1::deadbeef(sum, 0)
                }
                callinr 11223344(sum)
            "##,
            &["while", "calllib", "callstatic", "callinr", "if"],
        ),
        (
            r##"
                let numbers = [1, 2, 3]
                let info = map {
                    "numbers": numbers
                    "total": 3
                }
                append(numbers, 4)
                print numbers
                print info
            "##,
            &["map", "append", "print", "numbers", "total"],
        ),
        (
            r##"
                var x $0 = 42
                let aux = {
                    let inner = x
                    inner + 1
                }
                var y = aux
                let result = {
                    var staged = y
                    staged * x
                }
                print result
            "##,
            &["$0 = 42", "$1 =", "print ", "$2 * $0"],
        ),
    ];

    for (script, keywords) in scripts {
        check_fitsh_ir_roundtrip(script, keywords);
    }
}

#[test]
fn param_block_prints_from_ir_roundtrip() {
    let script = r##"
        param { addr, sat }
        print addr
        print sat
    "##;
    let (block, source_map) = lang_to_irnode_with_sourcemap(script).unwrap();
    let printed = irnode_to_lang_with_sourcemap(block, &source_map).unwrap();
    assert!(printed.contains("param { addr, sat }"));
}

fn find_print_expression(block: &IRNodeBlock) -> &Box<dyn IRNode> {
    block
        .iter()
        .find_map(|node| {
            if let Some(print) = node.as_any().downcast_ref::<IRNodeSingle>() {
                if print.inst == Bytecode::PRT {
                    return Some(&print.subx);
                }
            }
            None
        })
        .expect("expected `print` statement in block")
}

#[test]
fn expression_precedence_add_mul() {
    let script = r##"
        print 1 + 2 * 3 + 4
    "##;
    let block = lang_to_irnode(script).unwrap();
    let expr = find_print_expression(&block);

    let top_add = expr
        .as_any()
        .downcast_ref::<IRNodeDouble>()
        .expect("expected top-level addition");
    assert_eq!(top_add.inst, Bytecode::ADD);

    let left_add = top_add
        .subx
        .as_any()
        .downcast_ref::<IRNodeDouble>()
        .expect("expected nested addition on the left");
    assert_eq!(left_add.inst, Bytecode::ADD);

    let multiplication = left_add
        .suby
        .as_any()
        .downcast_ref::<IRNodeDouble>()
        .expect("expected multiplication as the right operand of the inner addition");
    assert_eq!(multiplication.inst, Bytecode::MUL);

    let left_mul = multiplication
        .subx
        .as_any()
        .downcast_ref::<IRNodeLeaf>()
        .expect("expected literal `2`");
    assert_eq!(left_mul.inst, Bytecode::P2);

    let right_mul = multiplication
        .suby
        .as_any()
        .downcast_ref::<IRNodeLeaf>()
        .expect("expected literal `3`");
    assert_eq!(right_mul.inst, Bytecode::P3);

    let right_const = top_add
        .suby
        .as_any()
        .downcast_ref::<IRNodeParam1>()
        .expect("expected constant `4`");
    assert_eq!(right_const.inst, Bytecode::PU8);
    assert_eq!(right_const.para, 4);
}

#[test]
fn expression_precedence_pow_right_assoc() {
    let script = r##"
        print 2 ** 3 ** 2
    "##;
    let block = lang_to_irnode(script).unwrap();
    let expr = find_print_expression(&block);

    let top_pow = expr
        .as_any()
        .downcast_ref::<IRNodeDouble>()
        .expect("expected top-level pow");
    assert_eq!(top_pow.inst, Bytecode::POW);

    let right_pow = top_pow
        .suby
        .as_any()
        .downcast_ref::<IRNodeDouble>()
        .expect("expected nested pow on the right");
    assert_eq!(right_pow.inst, Bytecode::POW);

    let inner_left = right_pow
        .subx
        .as_any()
        .downcast_ref::<IRNodeLeaf>()
        .expect("expected literal `3`");
    assert_eq!(inner_left.inst, Bytecode::P3);

    let inner_right = right_pow
        .suby
        .as_any()
        .downcast_ref::<IRNodeLeaf>()
        .expect("expected literal `2`");
    assert_eq!(inner_right.inst, Bytecode::P2);

    let top_left = top_pow
        .subx
        .as_any()
        .downcast_ref::<IRNodeLeaf>()
        .expect("expected literal `2`");
    assert_eq!(top_left.inst, Bytecode::P2);
}

#[test]
fn decompile_preserves_subtract_parens() {
    let script = r##"
        print 5 - (3 - 2)
    "##;
    let ircode = lang_to_ircode(script).unwrap();
    let printed = ircode_to_lang(&ircode).unwrap();
    assert!(printed.contains("5 - (3 - 2)"));
    assert!(printed.contains("print"));
}

#[test]
fn decompile_preserves_multiply_parens() {
    let script = r##"
        print 5 * (3 * 2)
    "##;
    let ircode = lang_to_ircode(script).unwrap();
    let printed = ircode_to_lang(&ircode).unwrap();
    assert!(printed.contains("5 * (3 * 2)"));
    assert!(printed.contains("print"));
}

#[test]
fn decompile_hacswap_sell_args_without_packlist() {
    let script = r##"
        lib HacSwap = 1: emqjNS9PscqdBpMtnC3Jfuc4mvZUPYTPS
        var sat = 4626909 as u64
        var zhu = HacSwap.sell(sat, 100000, 300)
    "##;
    let (ircode, smap) = lang_to_ircode_with_sourcemap(script).unwrap();
    let printed = ircode_to_lang_with_sourcemap(&ircode, &smap).unwrap();
    assert!(printed.contains("HacSwap.sell(sat, 0x0186a0 as u32, 300)"));
    assert!(!printed.contains("pack_list()"));
}

#[test]
fn decompile_native_transfer_args_flatten_cat() {
    let script = r##"
        var adr = address_ptr(1)
        var val = 12345 as u64
        transfer_sat_to(adr, val)
        transfer_hac_from(adr, zhu_to_hac(val))
    "##;
    let ircode = lang_to_ircode(script).unwrap();
    let printed = ircode_to_lang(&ircode).unwrap();
    assert!(printed.contains("transfer_sat_to($0, $1)"));
    assert!(printed.contains("transfer_hac_from($0, zhu_to_hac($1))"));
}

#[test]
fn decompile_with_sourcemap_lists_lib_defs_at_top() {
    let script = r##"
        lib HacSwap = 1: emqjNS9PscqdBpMtnC3Jfuc4mvZUPYTPS
        var sat = 100000000 as u64
        print sat
    "##;
    let (ircode, smap) = lang_to_ircode_with_sourcemap(script).unwrap();
    let printed = ircode_to_lang_with_sourcemap(&ircode, &smap).unwrap();
    assert!(printed.starts_with("lib HacSwap = 1: emqjNS9PscqdBpMtnC3Jfuc4mvZUPYTPS\n"));
}

#[test]
fn decompile_end_abort_as_keywords() {
    let script = r##"
        print 2
        abort
        end
    "##;
    let ircode = lang_to_ircode(script).unwrap();
    let printed = ircode_to_lang(&ircode).unwrap();
    assert!(printed.contains("abort"));
    assert!(printed.contains("end"));
    assert!(!printed.contains("abort()"));
    assert!(!printed.contains("end()"));
}

#[test]
fn decompile_local_vars_use_slot_names() {
    let script = r##"
        var foo = 123 as u64
        var bar = foo
        print bar
    "##;
    let (ircode, smap) = lang_to_ircode_with_sourcemap(script).unwrap();
    let printed = ircode_to_lang_with_sourcemap(&ircode, &smap).unwrap();
    assert!(printed.contains("print bar"));
    assert!(printed.contains("var foo"));
}
