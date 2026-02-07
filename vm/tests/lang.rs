// use sys::*;
// use vm::IRNode;
// use vm::rt::BytecodePrint;
// use vm::ir::IRCodePrint;
// use vm::lang::{Tokenizer, Syntax};

use vm::ir::*;
use vm::lang::*;
use vm::rt::*;
use vm::IRNode;
use vm::PrintOption;

mod common;

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
fn multi_return_ir_func_is_rejected_without_panic() {
    let script = r##"
        swap(1, 2)
    "##;

    let res = std::panic::catch_unwind(|| lang_to_irnode(script));
    assert!(res.is_ok(), "compiler panicked for multi-return ir func");
    assert!(res.unwrap().is_err());
}

#[test]
fn bind_slot_and_cache_print() {
    let script = r##"
        var x $0 = 1
        bind foo = $0
        bind bar = foo
        print bar
        print bar
    "##;
    let ircodes = lang_to_ircode(script).unwrap();
    let printed = ircode_to_lang(&ircodes).unwrap();
    assert!(printed.matches("print $0").count() >= 2);
}

#[test]
fn callview_callthis_print() {
    let script = r##"
        callview 2::abcdef01()
        callthis 0::11223344()
        callpure 3::deadbeef()
    "##;
    let ircodes = lang_to_ircode(script).unwrap();
    let printed = ircode_to_lang(&ircodes).unwrap();
    assert!(printed.contains("callview 2::0xabcdef01("));
    assert!(printed.contains("callthis 0::0x00ab4130("));
    assert!(printed.contains("callpure 3::0xdeadbeef("));
}

#[test]
fn callthis_callself_callsuper_print_and_roundtrip() {
    // 11223344 (decimal) == 0x00ab4130
    let script = r##"
        callthis 0::11223344(1)
        callself 0::11223344(2)
        callsuper 0::11223344(3)
    "##;
    let (ircd, smap) = lang_to_ircode_with_sourcemap(script).unwrap();
    let printed = format_ircode_to_lang(&ircd, Some(&smap)).unwrap();
    assert!(printed.contains("callthis 0::0x00ab4130("));
    assert!(printed.contains("callself 0::0x00ab4130("));
    assert!(printed.contains("callsuper 0::0x00ab4130("));

    // Also ensure the dot-call sugar is emitted when function names exist.
    let sugar = r##"
        this.foo(1)
        self.foo(2)
        super.foo(3)
    "##;
    let (ircd2, smap2) = lang_to_ircode_with_sourcemap(sugar).unwrap();
    let printed2 = format_ircode_to_lang(&ircd2, Some(&smap2)).unwrap();
    assert!(printed2.contains("/*callthis*/ this.foo("));
    assert!(printed2.contains("/*callself*/ self.foo("));
    assert!(printed2.contains("/*callsuper*/ super.foo("));
}

#[test]
fn call_keyword_print() {
    let script = r##"
        call 1::abcdef01()
    "##;
    let ircodes = lang_to_ircode(script).unwrap();
    let printed = ircode_to_lang(&ircodes).unwrap();
    assert!(printed.contains("call 1::0xabcdef01("));
}

#[test]
fn decompile_system_calls_preserve_default_empty_arg_unless_opted_in() {
    // Native call with 0 args (still consumes an argv buffer; compiler emits "" as placeholder).
    let script = r##"
        print context_address()
        print block_height()
    "##;

    let block = lang_to_irnode(script).unwrap();
    let plain = PrintOption::new("  ", 0);
    let printed_plain = Formater::new(&plain).print(&block);
    assert!(printed_plain.contains("context_address(\"\")"));
    // EXTENV(block_height) has metadata.input == 0; compiler emits no argv placeholder.
    assert!(printed_plain.contains("block_height()"));

    let mut pretty = PrintOption::new("  ", 0);
    pretty.hide_default_call_argv = true;
    let printed_pretty = Formater::new(&pretty).print(&block);
    assert!(printed_pretty.contains("context_address()"));
    assert!(printed_pretty.contains("block_height()"));
}

#[test]
fn decompile_contract_calls_hide_default_nil_only_when_opted_in() {
    let script = r##"
        call 1::abcdef01()
    "##;
    let block = lang_to_irnode(script).unwrap();

    let plain = PrintOption::new("  ", 0);
    let printed_plain = Formater::new(&plain).print(&block);
    assert!(printed_plain.contains("call 1::0xabcdef01(nil)"));

    let mut pretty = PrintOption::new("  ", 0);
    pretty.hide_default_call_argv = true;
    let printed_pretty = Formater::new(&pretty).print(&block);
    assert!(printed_pretty.contains("call 1::0xabcdef01()"));
}

#[test]
fn not_operator_parenthesizes_lower_precedence_operands_on_roundtrip() {
    // The parser uses IRNodeWrapOne to record explicit parentheses, but WrapOne is not
    // serialized into ircode. Decompilation must still print parentheses when required
    // by precedence, otherwise recompilation changes semantics.
    let script = r##"
        print !(1 == 1 && 2 == 2)
        print !(1 + 2 * 3 == 7)
    "##;
    let _ = common::checked_compile_fitsh_to_ir(script);
}

#[test]
fn pow_left_nested_requires_parentheses_on_roundtrip() {
    // `**` is right-associative in the parser, so a left-nested IR tree must be
    // printed with parentheses to preserve semantics.
    let script = r##"
        print (2 ** 3) ** 2
    "##;
    let _ = common::checked_compile_fitsh_to_ir(script);
}

#[test]
fn inline_block_expression_with_statements_roundtrips() {
    // Block expressions (IRBLOCKR) may contain multiple statements and a final value.
    // Decompilation must keep braces/newlines so the recompiled semantics match.
    let script = r##"
        var stmt = {
            print 5
            0
        }
        print stmt
    "##;
    let _ = common::checked_compile_fitsh_to_ir(script);
}

#[test]
fn call_args_preserve_block_expression_argument_boundaries() {
    // Block expressions may include newlines; decompilation must not split one argument
    // into many by line-joining.
    let script = r##"
        print sha3({
            print 1
            "abc"
        })

        transfer_hac_to(emqjNS9PscqdBpMtnC3Jfuc4mvZUPYTPS, {
            print 2
            1
        })
    "##;
    let _ = common::checked_compile_fitsh_to_ir(script);
}

#[test]
fn const_defs_not_injected_into_inline_expressions() {
    // Source-map const defs are emitted as a top-level prelude.
    // Inline printing (used for call args / expression formatting) must not
    // inject `const ...` lines into expression contexts.
    let script = r##"
        const ONE = 1

        // The compiler replaces `ONE` with literal `1` in IR;
        // decompilation should restore `ONE` and emit `const ONE = 1` once at top-level.
        print sha3(ONE + 2)
    "##;

    let (block, source_map) = lang_to_irnode_with_sourcemap(script).unwrap();
    let mut opt = PrintOption::new("    ", 0).with_source_map(&source_map);
    opt.recover_literals = true;
    let printed = Formater::new(&opt).print(&block);

    assert!(printed.contains("const ONE = 1"));
    assert_eq!(printed.matches("const ONE = 1").count(), 1);
    assert!(printed.contains("sha3(ONE + 2)"));
    assert!(!printed.contains("sha3(const"));

    let const_pos = printed.find("const ONE = 1").unwrap();
    let block_pos = printed.find('{').unwrap();
    assert!(const_pos < block_pos);
}

#[test]
fn inline_if_expression_argument_with_multiline_blocks_roundtrips() {
    // `if` expressions may print across multiple lines, but when used as a call argument
    // they flow through `print_inline()` which may flatten newlines.
    // This must remain parseable and preserve semantics.
    let script = r##"
        print sha3(if true {
            print 1
            "a"
        } else {
            print 2
            "b"
        })
    "##;

    let _ = common::checked_compile_fitsh_to_ir(script);
}

#[test]
fn inline_if_expression_argument_with_var_and_final_value_roundtrips() {
    // Stress `print_inline()` newline flattening: block bodies contain a declaration and a final value.
    // If newlines are flattened incorrectly, parsing can become ambiguous or fail.
    let script = r##"
        print sha3(if true {
            var x = 1
            x
        } else {
            2
        })
    "##;

    let _ = common::checked_compile_fitsh_to_ir(script);
}

#[test]
fn inline_multiline_map_expression_argument_roundtrips() {
    // `map { ... }` often prints across multiple lines; when used as a call argument it
    // flows through `print_inline()` and may have newlines flattened.
    let script = r##"
        print sha3(map {
            "a": {
                print 1
                "x"
            }
            "b": 2
        })
    "##;
    let _ = common::checked_compile_fitsh_to_ir(script);
}

#[test]
fn inline_multiline_list_expression_argument_roundtrips() {
    // Same as map: ensure list printing remains parseable when flattened for inline args.
    let script = r##"
        print sha3(list {
            {
                print 1
                "x"
            }
            2
        })
    "##;
    let _ = common::checked_compile_fitsh_to_ir(script);
}

#[test]
fn inline_if_expression_as_list_element_roundtrips() {
    // Nested inline contexts: list literal contains an `if` expression whose branches are blocks.
    // This exercises: print_inline(if) -> newline flattening, inside list { ... } which itself
    // becomes an inline call argument.
    let script = r##"
        print sha3(list {
            if true {
                var x = 1
                x
            } else {
                2
            }
        })
    "##;

    let _ = common::checked_compile_fitsh_to_ir(script);
}

#[test]
fn ir_func_call_arguments_parse_in_value_context() {
    // IR functions like `append(...)` previously parsed argv via `item_may_block(false)`
    // (statement context), which would make `if ... else ...` become `IRIF` (no retval)
    // and fail `checkretval()` when used as an argument.
    let script = r##"
        bind numbers = [1]
        append(numbers, if true { 2 } else { 3 })
        print numbers
    "##;

    let _ = common::checked_compile_fitsh_to_ir(script);
}

#[test]
fn log_arguments_parse_in_value_context() {
    // `log(...)` is an argument-taking instruction. Its arguments are value expressions.
    // Ensure constructs like `if ... else ...` are parsed as expression-form `IRIFR`.
    let script = r##"
        log(if true { 1 } else { 2 }, 0)
    "##;

    let ircodes = common::checked_compile_fitsh_to_ir(script);
    assert!(
        ircodes.iter().any(|b| *b == Bytecode::IRIFR as u8),
        "expected IRIFR for `if` used as log argument"
    );
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
    let opt = PrintOption::new("    ", 0).with_source_map(&source_map);
    let printed = Formater::new(&opt).print(&block);
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
fn let_cannot_reassign() {
    let script = r##"
        let x = 1
        x = 2
    "##;
    let err = lang_to_ircode(script).unwrap_err();
    assert!(err.contains("cannot assign to immutable symbol 'x'"));
}

#[test]
fn var_initializer_must_be_value_expression() {
    // `bind` is a declaration/statement (returns no value). Allowing it as a var initializer
    // would compile into `PUT` without a stack value, causing stack/semantic issues.
    let script = r##"
        var x = bind y = 1
    "##;
    let err = lang_to_ircode(script).unwrap_err();
    assert!(err.contains("initializer") || err.contains("return"));
}

#[test]
fn return_argument_must_be_value_expression() {
    let script = r##"
        return end
    "##;
    let err = lang_to_ircode(script).unwrap_err();
    assert!(err.contains("return") || err.contains("return value"));
}

#[test]
fn assert_argument_must_be_value_expression() {
    let script = r##"
        assert end
    "##;
    let err = lang_to_ircode(script).unwrap_err();
    assert!(err.contains("assert") || err.contains("return value"));
}

#[test]
fn throw_argument_must_be_value_expression() {
    let script = r##"
        throw end
    "##;
    let err = lang_to_ircode(script).unwrap_err();
    assert!(err.contains("throw") || err.contains("return value"));
}

#[test]
fn binary_operator_operands_must_be_value_expressions() {
    // Binary operators consume values from the stack.
    let script = r##"
        1 + end
    "##;
    let err = lang_to_ircode(script).unwrap_err();
    assert!(err.contains("return value") || err.contains("not have return value"));
}

#[test]
fn is_operator_lhs_must_be_value_expression() {
    // `is` consumes a value from the stack; lhs must return a value.
    let script = r##"
        end is nil
    "##;
    let err = lang_to_ircode(script).unwrap_err();
    assert!(err.contains("return value") || err.contains("not have return value"));
}

#[test]
fn item_get_index_must_be_value_expression() {
    let script = r##"
        var arr = list { 1 }
        print arr[end]
    "##;
    let err = lang_to_ircode(script).unwrap_err();
    assert!(err.contains("return value") || err.contains("not have return value"));
}

#[test]
fn else_if_statement_allows_statement_branches() {
    // In statement context, else-if chains should parse as IRIF (not IRIFR),
    // so branch blocks may be statement-only.
    let script = r##"
        if true {
            print 1
        } else if true {
            print 2
        } else {
            print 3
        }
        end
    "##;
    let _ = common::checked_compile_fitsh_to_ir(script);
}

#[test]
fn else_if_expression_still_requires_value_branches() {
    // In expression context, every branch in the else-if chain must return a value.
    let script = r##"
        print if true {
            1
        } else if true {
            print 2
        } else {
            3
        }
    "##;
    let err = lang_to_ircode(script).unwrap_err();
    assert!(err.contains("block expression") || err.contains("return value"));
}

#[test]
fn if_expression_requires_else_branch_at_parse_time() {
    let script = r##"
        print if true { 1 }
    "##;
    let err = lang_to_ircode(script).unwrap_err();
    assert!(err.contains("if expression must have else branch"));
}

#[test]
fn item_get_if_expression_index_roundtrips() {
    let script = r##"
        var arr = [10, 20]
        print arr[if true {
            0
        } else {
            1
        }]
    "##;
    let _ = common::checked_compile_fitsh_to_ir(script);
}

#[test]
fn item_get_receiver_list_literal_roundtrips() {
    // Indexing should work on any value expression, including list literals.
    let script = r##"
        var x = [1, 2][0]
        print x
    "##;
    let _ = common::checked_compile_fitsh_to_ir(script);
}

#[test]
fn item_get_receiver_call_result_roundtrips() {
    // Indexing should work on call results (receiver isn't an identifier).
    let script = r##"
        var b = sha3("abc")[0]
        print b
    "##;
    let _ = common::checked_compile_fitsh_to_ir(script);
}

#[test]
fn item_get_receiver_binary_expression_preserves_precedence_roundtrips() {
    // Formatter must emit parentheses: `(a + b)[0]` must not decompile as `a + b[0]`.
    let script = r##"
        var a = [1]
        var b = [2]
        var x = (a + b)[0]
        print x
    "##;
    let _ = common::checked_compile_fitsh_to_ir(script);
}

#[test]
fn item_get_receiver_bind_symbol_roundtrips() {
    // `bind` symbols are value expressions but not slots; indexing must still work.
    let script = r##"
        bind arr = [7, 9]
        print arr[1]
    "##;
    let _ = common::checked_compile_fitsh_to_ir(script);
}

#[test]
fn map_value_if_expression_roundtrips() {
    let script = r##"
        var flag = true
        let obj = map {
            "v": if flag {
                1
            } else {
                2
            }
        }
        print obj
    "##;
    let _ = common::checked_compile_fitsh_to_ir(script);
}

#[test]
fn var_without_reassign_prints_as_let() {
    let script = r##"
        var x $0 = 1
        print x
    "##;
    let (block, source_map) = lang_to_irnode_with_sourcemap(script).unwrap();
    let printed = irnode_to_lang_with_sourcemap(block, &source_map).unwrap();
    assert!(printed.contains("let x $0 ="));
}

#[test]
fn bind_var_interleave_print() {
    let script = r##"
        var x $0 = 10
        bind aux = x
        var y = aux
        bind cache = y
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
fn print_decomp_bind_alias_clones_expression() {
    let script = r##"
        bind base = {
            if true {
                { 1 }
            } else {
                { 2 }
            }
        }
        bind alias = base
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
        // Ensure we still emit a real IRBLOCKR that cannot be simplified away.
        print {
            print 9
            10
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
    assert!(!ircodes.iter().any(|b| *b == Bytecode::IRBLOCKR as u8));
    assert!(!ircodes.iter().any(|b| *b == Bytecode::IRIFR as u8));
}

#[test]
fn if_expression_single_stmt_branches_elide_irblockr_in_ircode() {
    // Deprecated: we require ircode byte-for-byte stability, so single-statement
    // block wrappers must remain representable. Keep a sanity check that the
    // expression opcode form is used.
    let script = r##"
        print if true { 3 } else { 4 }
    "##;
    let ircodes = lang_to_ircode(script).unwrap();
    assert!(ircodes.iter().any(|b| *b == Bytecode::IRIFR as u8));
    assert!(ircodes.iter().any(|b| *b == Bytecode::IRBLOCKR as u8));
}

#[test]
fn if_statement_and_while_single_stmt_bodies_elide_irblock_in_ircode() {
    // Deprecated: ircode stability requires preserving IRBLOCK wrappers.
    let script = r##"
        if true { print 1 } else { print 2 }
        while true { print 3 }
    "##;
    let ircodes = lang_to_ircode(script).unwrap();
    assert!(ircodes.iter().any(|b| *b == Bytecode::IRIF as u8));
    assert!(ircodes.iter().any(|b| *b == Bytecode::IRWHILE as u8));
    assert!(ircodes.iter().any(|b| *b == Bytecode::IRBLOCK as u8));
}

#[test]
fn nested_expression_contexts_emit_expr_opcodes() {
    let script = r##"
        print {
            if true {
                bind inner = if false {
                    { if false { 10 } else { 11 } }
                } else {
                    { { 12 } }
                }
                inner
            } else {
                {
                    bind deep = { if true { { 13 } } else { { 14 } } }
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
    assert!(blockr >= 1);
    assert!(ifr >= 4);
}

#[test]
fn print_options_on_off_preserve_ircode_bytes() {
    // This script intentionally hits multiple formatter options:
    // - has a cast opcode (so `recover_literals` must not drop it)
    // - has default argv placeholders (so `hide_default_call_argv` must be lossless)
    // - has list literal (so `flatten_array_list` must be lossless)
    // - has if/while blocks (so bracing must preserve container opcodes)
    let script = r##"
        param { amt }
        print amt as u8
        call 1::abcdef01()
        print context_address("")
        print [1, 2]
        if true { print 3 } else { print 4 }
        while false { print 5 }
    "##;

    let (ircode, smap) = lang_to_ircode_with_sourcemap(script).unwrap();
    let mut seek = 0;
    let block = parse_ir_block(&ircode, &mut seek).unwrap();
    assert_eq!(seek, ircode.len());

    // All options off.
    let plain = PrintOption::new("  ", 0).with_source_map(&smap);
    let plain_text = Formater::new(&plain).print(&block);
    let plain_ir = lang_to_ircode(&plain_text).unwrap();
    assert_eq!(ircode, plain_ir);

    // All options on (mirrors `format_ircode_to_lang`).
    let mut pretty = PrintOption::new("  ", 0).with_source_map(&smap);
    pretty.trim_root_block = true;
    pretty.trim_head_alloc = true;
    pretty.trim_param_unpack = true;
    pretty.hide_default_call_argv = true;
    pretty.call_short_syntax = true;
    pretty.flatten_call_list = true;
    pretty.flatten_array_list = true;
    pretty.flatten_syscall_cat = true;
    pretty.recover_literals = true;

    let pretty_text = Formater::new(&pretty).print(&block);
    let pretty_ir = lang_to_ircode(&pretty_text)
        .map_err(|e| format!("{}\n---- pretty_text ----\n{}\n---------------------\n", e, pretty_text))
        .unwrap();
    assert_eq!(ircode, pretty_ir);
}

#[test]
fn print_option_each_toggle_and_sourcemap_on_off_preserve_ircode_bytes() {
    // This script includes a param-unpack and a local use, so sourcemap-less decompilation
    // must still emit a compilable `param { ... }` and preserve byte-for-byte ircode.
    let script = r##"
        param { amt }
        print amt as u8
        call 1::abcdef01()
        print context_address("")
        print [1, 2]
        if true { print 3 } else { print 4 }
        while false { print 5 }
    "##;

    let (ircode, smap) = lang_to_ircode_with_sourcemap(script).unwrap();
    let mut seek = 0;
    let block = parse_ir_block(&ircode, &mut seek).unwrap();
    assert_eq!(seek, ircode.len());

    #[derive(Clone, Copy, Debug)]
    enum OptKey {
        EmitLibPrelude,
        TrimRootBlock,
        TrimHeadAlloc,
        TrimParamUnpack,
        HideDefaultCallArgv,
        CallShortSyntax,
        FlattenCallList,
        FlattenArrayList,
        FlattenSyscallCat,
        RecoverLiterals,
    }

    fn set_opt(opt: &mut PrintOption, key: OptKey, val: bool) {
        match key {
            OptKey::EmitLibPrelude => opt.emit_lib_prelude = val,
            OptKey::TrimRootBlock => opt.trim_root_block = val,
            OptKey::TrimHeadAlloc => opt.trim_head_alloc = val,
            OptKey::TrimParamUnpack => opt.trim_param_unpack = val,
            OptKey::HideDefaultCallArgv => opt.hide_default_call_argv = val,
            OptKey::CallShortSyntax => opt.call_short_syntax = val,
            OptKey::FlattenCallList => opt.flatten_call_list = val,
            OptKey::FlattenArrayList => opt.flatten_array_list = val,
            OptKey::FlattenSyscallCat => opt.flatten_syscall_cat = val,
            OptKey::RecoverLiterals => opt.recover_literals = val,
        }
    }

    let keys = [
        OptKey::EmitLibPrelude,
        OptKey::TrimRootBlock,
        OptKey::TrimHeadAlloc,
        OptKey::TrimParamUnpack,
        OptKey::HideDefaultCallArgv,
        OptKey::CallShortSyntax,
        OptKey::FlattenCallList,
        OptKey::FlattenArrayList,
        OptKey::FlattenSyscallCat,
        OptKey::RecoverLiterals,
    ];

    for map_enabled in [false, true] {
        for key in keys {
            for val in [false, true] {
                let mut opt = PrintOption::new("  ", 0);
                if map_enabled {
                    opt.map = Some(&smap);
                }
                set_opt(&mut opt, key, val);
                let text = Formater::new(&opt).print(&block);
                let ir2 = lang_to_ircode(&text)
                    .map_err(|e| {
                        format!(
                            "{}\n---- printed (map_enabled={}, opt={:?}={}) ----\n{}\n---------------------\n",
                            e, map_enabled, key, val, text
                        )
                    })
                    .unwrap();
                assert_eq!(
                    ircode, ir2,
                    "ircode mismatch (map_enabled={}, opt={:?}={})\n{}",
                    map_enabled, key, val, text
                );
            }
        }
    }
}

#[test]
fn var_rhs_block_expression_emits_expr_opcodes() {
    let script = r##"
        var holder = {
            if true {
                bind inner = if false {
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
                bind sum = {
                    var builder = counter
                    builder += amt
                    builder
                }
                if sum > 0 {
                    callview 1::abcdef01(sum)
                } else {
                    callpure 1::deadbeef(sum, 0)
                }
                callthis 0::11223344(sum)
            "##,
            &["while", "callview", "callpure", "callthis", "if"],
        ),
        (
            r##"
                bind numbers = [1, 2, 3]
                bind info = map {
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
                bind aux = {
                    bind inner = x
                    inner + 1
                }
                var y = aux
                bind result = {
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
    let (block, source_map) = lang_to_ircode_with_sourcemap(script).unwrap();
    let printed = format_ircode_to_lang(&block, Some(&source_map)).unwrap();
    assert!(printed.contains("param { addr, sat }"));
}

#[test]
fn reject_unclosed_log_argument_delimiter() {
    let script = r##"
        log(1 2
    "##;
    let err = lang_to_ircode(script).unwrap_err();
    assert!(err.contains("log argv number error"));
}

#[test]
fn reject_unclosed_if_block() {
    let script = r##"
        if true {
            print 1
        end
    "##;
    let err = lang_to_ircode(script).unwrap_err();
    assert!(err.contains("block format error"));
}

#[test]
fn reject_binary_not_operator() {
    let script = r##"
        1 ! 0
    "##;
    let err = lang_to_ircode(script).unwrap_err();
    assert!(err.contains("cannot be binary"));
}

#[test]
fn reject_param_block_non_identifier_member() {
    let script = r##"
        param { a 1 }
    "##;
    let err = lang_to_ircode(script).unwrap_err();
    assert!(err.contains("param format error"));
}

#[test]
fn reject_param_block_without_close_brace() {
    let script = r##"
        param { a
    "##;
    let err = lang_to_ircode(script).unwrap_err();
    assert!(err.contains("param format error"));
}

#[test]
fn reject_param_as_value_expression() {
    let script = r##"
        print (param { a })
    "##;
    let err = lang_to_ircode(script).unwrap_err();
    assert!(err.contains("return value") || err.contains("format error"));
}

fn find_print_expression(block: &IRNodeArray) -> &Box<dyn IRNode> {
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
fn decompile_hacswap_sell_args_without_list() {
    let script = r##"
        lib HacSwap = 1: emqjNS9PscqdBpMtnC3Jfuc4mvZUPYTPS
        var sat = 4626909 as u64
        var zhu = HacSwap.sell(sat, 100000, 300)
    "##;
    let (ircode, smap) = lang_to_ircode_with_sourcemap(script).unwrap();
    let printed = format_ircode_to_lang(&ircode, Some(&smap)).unwrap();
    // panic!("{}", printed);
    assert!(printed.contains("HacSwap.sell(sat, 100000, 300)"));
    assert!(!printed.contains("pack_list {"));
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
    assert!(printed.contains("transfer_sat_to($0 ++ $1)"));
    assert!(printed.contains("transfer_hac_from($0 ++ zhu_to_hac($1))"));
}

#[test]
fn format_ircode_rehydrates_numeric_literal() {
    let script = r##"
        print 70000 as u32
    "##;
    let (ircode, smap) = lang_to_ircode_with_sourcemap(script).unwrap();
    let formatted = format_ircode_to_lang(&ircode, Some(&smap)).unwrap();
    assert!(formatted.contains("70000"));
    assert!(!formatted.contains("as u32"));
}

#[test]
fn format_ircode_preserves_mismatched_cast() {
    let script = r##"
        print 1 as u64
    "##;
    let (ircode, smap) = lang_to_ircode_with_sourcemap(script).unwrap();
    let formatted = ircode_to_lang_with_sourcemap(&ircode, &smap).unwrap();
    assert!(formatted.contains("1 as u64"));
}

#[test]
fn list_keyword_roundtrip() {
    let script = r##"
        print [1, 2, 3]
    "##;
    let block = lang_to_irnode(script).unwrap();
    let mut opt = PrintOption::new("  ", 0);
    opt.call_short_syntax = true;
    opt.flatten_array_list = true;
    let printed = Formater::new(&opt).print(&block);
    assert!(printed.contains("[1, 2, 3]"));
    assert!(lang_to_ircode(&printed).is_ok());

    let default_printed = irnode_to_lang(block.clone()).unwrap();
    assert!(default_printed.contains("list {"));
    assert!(lang_to_ircode(&default_printed).is_ok());
}

#[test]
fn list_keyword_literal_compiles() {
    let script = r##"
        var arr = list { 1 2 3 }
        print arr
    "##;
    assert!(lang_to_ircode(script).is_ok());
}

#[test]
fn list_keyword_empty_newlist() {
    let script = r##"
        var arr = list { }
    "##;
    let codes = lang_to_ircode(script).unwrap();
    assert!(codes.contains(&(Bytecode::NEWLIST as u8)));
}

#[test]
fn call_short_syntax_uses_comment_short_form() {
    let script = r##"
        lib HacSwap = 1: emqjNS9PscqdBpMtnC3Jfuc4mvZUPYTPS
        var sat = 4626909 as u64
        var zhu = HacSwap.sell(sat, 100000, 300)
        print zhu
    "##;
    let (block, smap) = lang_to_irnode_with_sourcemap(script).unwrap();
    let mut opt = PrintOption::new("  ", 0);
    opt.map = Some(&smap);
    opt.trim_root_block = true;
    opt.trim_head_alloc = true;
    opt.trim_param_unpack = true;
    opt.call_short_syntax = true;
    let printed = Formater::new(&opt).print(&block);
    assert!(printed.contains("/*call*/ HacSwap.sell("));
    assert!(printed.contains("print"));
}

#[test]
fn empty_array_generates_newlist() {
    let script = r##"
        var arr = []
    "##;
    let codes = lang_to_ircode(script).unwrap();
    assert!(codes.contains(&(Bytecode::NEWLIST as u8)));
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
fn sourcemap_lib_prelude_not_injected_into_inline_blocks() {
    let script = r##"
        lib HacSwap = 1: emqjNS9PscqdBpMtnC3Jfuc4mvZUPYTPS
        // block used as call argument must not receive `lib ...` prelude.
        print sha3({
            print 1
            "abc"
        })
    "##;
    let (block, smap) = lang_to_irnode_with_sourcemap(script).unwrap();
    let opt = PrintOption::new("  ", 0).with_source_map(&smap);
    let printed = Formater::new(&opt).print(&block);
    assert_eq!(printed.matches("lib HacSwap = 1:").count(), 1);
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
    let printed = format_ircode_to_lang(&ircode, Some(&smap)).unwrap();
    // panic!("{}", printed);
    assert!(printed.contains("print bar"));
    assert!(printed.contains("let foo"));
}
