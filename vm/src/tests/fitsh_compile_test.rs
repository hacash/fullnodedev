// Comprehensive Fitsh Language Compiler Tests Tests cover all syntax features and nested expression scenarios. NOTE: All comments and messages in this file are in English.

#[cfg(test)]
mod fitsh_compile_tests {

    use crate::lang::irnode_to_lang;
    use crate::lang::lang_to_bytecode;
    use crate::lang::lang_to_irnode;
    use crate::lang::lang_to_irnode_with_sourcemap;

    macro_rules! assert_compile_ok {
        ($($name:ident: $script:expr),* $(,)?) => {
            $(
                #[test]
                fn $name() {
                    let result = lang_to_irnode($script);
                    assert!(result.is_ok(), "Failed to compile: {}\nError: {:?}", $script, result.err());
                }
            )*
        };
    }

    macro_rules! assert_compile_err {
        ($($name:ident: $script:expr),* $(,)?) => {
            $(
                #[test]
                fn $name() {
                    let result = lang_to_irnode($script);
                    assert!(result.is_err(), "Should fail to compile: {}", $script);
                }
            )*
        };
    }

    macro_rules! assert_compile_err_contains {
        ($($name:ident: $script:expr, $needle:expr),* $(,)?) => {
            $(
                #[test]
                fn $name() {
                    let err = lang_to_irnode($script)
                        .expect_err(&format!("Should fail to compile: {}", $script));
                    let text = format!("{:?}", err);
                    assert!(
                        text.contains($needle),
                        "Error mismatch for script: {}\nExpected substring: {}\nActual: {}",
                        $script,
                        $needle,
                        text
                    );
                }
            )*
        };
    }

    macro_rules! assert_roundtrip {
        ($($name:ident: $script:expr),* $(,)?) => {
            $(
                #[test]
                fn $name() {
                    let ir = lang_to_irnode($script).expect(&format!("Failed to compile: {}", $script));
                    let decompiled = irnode_to_lang(ir).expect(&format!("Failed to decompile: {}", $script));
                    let ir2 = lang_to_irnode(&decompiled).expect(&format!("Failed to recompile: {}", $script));
                    let decompiled2 = irnode_to_lang(ir2).expect(&format!("Failed to decompile again: {}", $script));
                    let norm1 = decompiled.replace("\n\n", "\n");
                    let norm2 = decompiled2.replace("\n\n", "\n");
                    assert_eq!(norm1, norm2, "Roundtrip should be stable for: {}", $script);
                }
            )*
        };
    }

    // ========================================================================= 1. LITERALS - Integer, String, Character, Boolean, Nil, Address =========================================================================

    mod literals {
        use super::*;

        assert_compile_ok!(
            int_zero: "return 0",
            int_one: "return 1",
            int_large: "return 123456789",
            int_hex: "return 0xABCDEF",
            int_hex_lower: "return 0xabcdef",
            int_binary: "return 0b11110000",
            int_with_underscore: "return 1_000_000",
            str_empty: "return \"\"",
            str_simple: "return \"hello\"",
            str_with_spaces: "return \"hello world\"",
            str_with_escape: "return \"hello\\nworld\"",
            str_with_tab: "return \"hello\\tworld\"",
            str_with_quotes: "return \"hello \\\"world\\\"\"",
            str_unicode: "return \"你好\"",
            char_a: "return 'A'",
            char_digit: "return '0'",
            char_space: "return ' '",
            char_newline: "return '\\n'",
            char_tab: "return '\\t'",
            char_escape: "return '\\\\'",
            bool_true: "return true",
            bool_false: "return false",
            nil_literal: "return nil",
            address_literal: "return emqjNS9PscqdBpMtnC3Jfuc4mvZUPYTPS",
            num_u8: "return 100u8",
            num_u16: "return 100u16",
            num_u32: "return 100u32",
            num_u64: "return 100u64",
            num_u128: "return 100u128",
        );

        assert_roundtrip!(
            rt_int: "return 123",
            rt_str: "return \"hello\"",
            rt_char: "return 'A'",
            rt_bool: "return true",
            rt_nil: "return nil",
        );
    }

    // ========================================================================= 2. ARRAYS AND LISTS =========================================================================

    mod arrays_and_lists {
        use super::*;

        assert_compile_ok!(
            array_single: "return [1]",
            array_two: "return [1, 2]",
            array_three: "return [1, 2, 3]",
            array_empty: "return []",
            array_mixed: "return [1, \"hello\", true]",
            list_single: "return list { 1 }",
            list_two: "return list { 1 2 }",
            list_three: "return list { 1 2 3 }",
            list_empty: "return list { }",
            deep_nested: "return [[[1]], [[2]]]",
            array_in_array: "return [[1, 2, 3]]",
            array_expr: "return [1 + 2, 3 * 4]",
            array_var: "var x = 1\nreturn [x, x + 1]",
        );

        assert_roundtrip!(
            rt_array: "return [1, 2, 3]",
            rt_list: "return list { 1 2 3 }",
            rt_nested: "return [[[1]], [[2]]]",
        );
    }

    // ========================================================================= 3. MAPS =========================================================================

    mod maps {
        use super::*;

        assert_compile_ok!(
            map_single: "return map { \"key\": \"value\" }",
            map_two: "return map { \"a\": \"b\", \"c\": \"d\" }",
            map_empty: "return map { }",
            map_int_key: "return map { 1: 2 }",
            map_mixed_keys: "return map { \"str\": 1, 2: \"val\" }",
            map_addr_key: "return map { emqjNS9PscqdBpMtnC3Jfuc4mvZUPYTPS: 100 }",
            nested_map: "return map { \"outer\": map { \"inner\": 1 } }",
            map_in_array: "return [map { \"a\": 1 }, map { \"b\": 2 }]",
        );

        assert_roundtrip!(
            rt_map: "return map { \"k\": \"v\" }",
            rt_map_empty: "return map { }",
            rt_map_nested: "return map { \"a\": map { \"b\": 1 } }",
        );
    }

    // ========================================================================= 4. OPERATORS =========================================================================

    mod operators {
        use super::*;

        assert_compile_ok!(
            op_add: "return 1 + 2",
            op_sub: "return 5 - 3",
            op_mul: "return 2 * 3",
            op_div: "return 6 / 2",
            op_mod: "return 7 % 3",
            op_pow: "return 2 ** 3",
            op_shl: "return 1 << 4",
            op_shr: "return 16 >> 2",
            op_and: "return 5 & 3",
            op_or: "return 5 | 3",
            op_xor: "return 5 ^ 3",
            op_eq: "return 1 == 1",
            op_neq: "return 1 != 2",
            op_lt: "return 1 < 2",
            op_le: "return 1 <= 1",
            op_gt: "return 2 > 1",
            op_ge: "return 1 >= 1",
            op_and_bool: "return true && false",
            op_or_bool: "return true || false",
            op_not: "return !false",
            op_not_true: "return !true",
            concat_str: "return \"hello\" ++ \"world\"",
            concat_bytes: "return 0xAA ++ 0xBB",
            concat_mixed: "return \"a\" ++ \"b\" ++ \"c\"",
            compound_add: "var x = 1\nx += 1\nreturn x",
            compound_sub: "var x = 5\nx -= 2\nreturn x",
            compound_mul: "var x = 3\nx *= 2\nreturn x",
            compound_div: "var x = 6\nx /= 2\nreturn x",
            precedence_add_mul: "return 1 + 2 * 3",
            precedence_parens: "return (1 + 2) * 3",
            precedence_complex: "return 1 + 2 * 3 - 4 / 2",
        );

        assert_roundtrip!(
            rt_arith: "return 1 + 2 * 3",
            rt_logical: "return true && false || true",
            rt_concat: "return \"a\" ++ \"b\"",
        );
    }

    // ========================================================================= 5. NESTED EXPRESSIONS =========================================================================

    mod nested_expressions {
        use super::*;

        assert_compile_ok!(
            nest_arith_1: "return ((1 + 2) * 3) ** (4 - 2)",
            nest_arith_2: "return (1 + (2 * 3)) - (4 / 2)",
            nest_arith_3: "return (((1 + 2) + 3) + 4) + 5",
            nest_arith_4: "return 1 + 2 + 3 + 4 + 5",
            nest_logical_2: "return (true && false) || (true || false)",
            nest_func_1: "return length(append(list { 1 2 }, 3))",
            nest_func_3: "return head(append(list { 1 2 3 }, 4))",
            nest_cast_1: "return (1 as u64) as u128",
            nest_mixed_4: "return [1, 2, 3] ++ list { 4 5 }",
            nest_deep_1: "return (((((1)))))",
            nest_deep_2: "return [[[[1]]]]",
            nest_deep_3: "return map { \"a\": map { \"b\": map { \"c\": 1 } } }",
            nest_complex_1: "return length(list { 1 + 2, 3 * 4, 5 - 6 })",
            nest_complex_3: "return head(append(list { 1 2 3 }, (4 + 5) * 2))",
        );
    }

    // ========================================================================= 6. CONTROL FLOW =========================================================================

    mod control_flow {
        use super::*;

        assert_compile_ok!(
            if_simple: "if true { return 1 }\nreturn 0",
            if_else: "if true { return 1 } else { return 0 }",
            if_no_value: "if 1 > 0 { var x = 1 }\nreturn 0",
            if_elseif: "if 1 > 2 { return 1 } else if 2 > 3 { return 2 } else { return 3 }",
            if_elseif_multiple: "if 1 > 10 { return 1 } else if 2 > 10 { return 2 } else if 3 > 10 { return 3 } else { return 0 }",
            if_expr: "return if 1 > 0 { 1 } else { 0 }",
            if_expr_elseif: "return if false { 1 } else if true { 2 } else { 3 }",
            while_simple: "var i = 0\nwhile i < 10 { i += 1 }\nreturn i",
            while_nested: "var i = 0\nwhile i < 3 { var j = 0\n while j < 3 { j += 1 }\n i += 1 }\nreturn i",
            break_simple: "var i = 0\nwhile i < 10 { if i == 5 { break } i += 1 }\nreturn i",
            break_nested: "var i = 0\nwhile i < 10 { var j = 0\n while j < 10 { if j == 3 { break } j += 1 }\n if j == 3 { break }\n i += 1 }\nreturn i",
            break_in_if: "var sum = 0\nvar i = 0\nwhile i < 100 { i += 1\n if i > 10 { break }\n sum += i }\nreturn sum",
            continue_simple: "var i = 0\nwhile i < 10 { i += 1\n if i == 5 { continue } }\nreturn i",
            continue_skip: "var sum = 0\nvar i = 0\nwhile i < 10 { i += 1\n if i % 2 == 0 { continue }\n sum += i }\nreturn sum",
            continue_nested: "var i = 0\nwhile i < 5 { i += 1\n var j = 0\n while j < 5 { j += 1\n  if j == 2 { continue }\n } }\nreturn i",
            break_continue_combo: "var i = 0\nwhile i < 20 { i += 1\n if i == 3 { continue }\n if i > 8 { break }\n}\nreturn i",
            return_simple: "return 1",
            return_expr: "return 1 + 2",
            return_var: "var x = 1\nreturn x",
            assert_true: "assert true",
            assert_expr: "assert 1 + 1 == 2",
            assert_fail: "assert false",
            throw_simple: "throw \"error\"",
            throw_expr: "throw \"error: \" ++ \"message\"",
            abort_simple: "abort",
            end_simple: "end",
            block_expr: "var result = { var inner = 10\n inner + 1 }\nreturn result",
            block_nested: "return { { { 1 + 2 } } }",
        );

        assert_compile_err_contains!(
            err_break_outside_loop: "break", "break can only be used inside while loop",
            err_continue_outside_loop: "continue", "continue can only be used inside while loop",
            err_break_expr_context: "return if true { break } else { 1 }", "break statement cannot be used as expression",
            err_continue_expr_context: "return if true { continue } else { 1 }", "continue statement cannot be used as expression",
        );

        assert_roundtrip!(
            rt_if: "if true { return 1 } else { return 0 }",
            rt_while: "var i = 0\nwhile i < 10 { i += 1 }\nreturn i",
        );
    }

    // ========================================================================= 7. VARIABLE DECLARATIONS =========================================================================

    mod variable_declarations {
        use super::*;

        assert_compile_ok!(
            var_simple: "var x = 1\nreturn x",
            var_reassign: "var x = 1\nx = 2\nreturn x",
            var_multiple: "var x = 1\nvar y = 2\nreturn x + y",
            let_simple: "let x = 1\nreturn x",
            let_multiple: "let x = 1\nlet y = 2\nreturn x + y",
            var_explicit_slot: "var x $5 = 1\nreturn x",
            let_explicit_slot: "let x $3 = 1\nreturn x",
            bind_simple: "bind x = 1 + 2\nreturn x",
            bind_expr: "bind key = \"prefix_\" ++ \"suffix\"\nreturn key",
            const_int: "const X = 100\nreturn X",
            const_str: "const MSG = \"hello\"\nreturn MSG",
            const_bytes: "const DATA = 0xABCD\nreturn DATA",
            var_let_mix: "var x = 1\nlet y = 2\nreturn x + y",
        );

        assert_roundtrip!(
            rt_var: "var x = 1\nreturn x",
            rt_let: "let x = 1\nreturn x",
        );
    }

    // ========================================================================= 8. PARAM UNPACK =========================================================================

    mod param_unpack {
        use super::*;

        assert_compile_ok!(
            param_single: "param { x }\nreturn x",
            param_two: "param { a b }\nreturn a + b",
            param_three: "param { x y z }\nreturn x + y + z",
            param_body: "param { x }\nvar y = x + 1\nreturn y",
            param_with_var: "param { x }\nvar y = x\nreturn y + 1",
        );
    }

    // ========================================================================= 9. SPECIAL SYNTAX =========================================================================

    mod special_syntax {
        use super::*;

        assert_compile_ok!(
            bytecode_simple: "bytecode { POP }",
            bytecode_multiple: "bytecode { POP DUP SWAP }",
            log_two: "log(1, 2)",
            log_three: "log(1, 2, 3)",
            log_four: "log(1, 2, 3, 4)",
            log_five: "log(1, 2, 3, 4, 5)",
            log_bracket: "log[1, 2, 3]",
            log_brace: "log{1, 2, 3}",
            callcode_simple: "callcode 0::0xabcdef01\nend",
        );
    }

    // ========================================================================= 10. TYPE SYSTEM =========================================================================

    mod type_system {
        use super::*;

        assert_compile_ok!(
            as_u8: "return 1 as u8",
            as_u16: "return 1 as u16",
            as_u32: "return 1 as u32",
            as_u64: "return 1 as u64",
            as_u128: "return 1 as u128",
            as_bytes: "return 1 as bytes",
            as_address: "return 0xABCD as address",
            is_nil: "return nil is nil",
            is_not_nil: "return 1 is not nil",
            is_list: "return [] is list",
            is_map: "return map { } is map",
            is_bool: "return true is bool",
            is_u8: "return 1 is u8",
            is_u64: "return 1 is u64",
            is_bytes: "return \"\" is bytes",
            is_address: "return emqjNS9PscqdBpMtnC3Jfuc4mvZUPYTPS is address",
            is_not_list: "return [] is not list",
            is_not_map: "return map { } is not map",
        );

        assert_roundtrip!(
            rt_as: "var x = 1\nreturn x as u64",
            rt_is: "var x = nil\nreturn x is nil",
        );
    }

    // ========================================================================= 11. SLOT REFERENCES =========================================================================

    mod slot_references {
        use super::*;

        assert_compile_ok!(
            slot_read_0: "param { x }\nreturn $0",
            slot_read_1: "param { a b }\nreturn $1",
            slot_read_10: "var x $10 = 1\nreturn $10",
            slot_write_0: "param { x }\n$0 = 999\nreturn $0",
            slot_write_1: "param { a b }\n$1 = 888\nreturn $1",
            var_slot: "var opt $10 = 123\nreturn opt",
            let_slot: "let val $5 = 100\nreturn val",
            slot_with_var: "param { x }\nvar y = $0 + 1\nreturn y",
        );
    }

    // ========================================================================= 12. BUILT-IN FUNCTIONS =========================================================================

    mod builtin_functions {
        use super::*;

        assert_compile_ok!(
            func_sha2: "return sha2(\"hello\")",
            func_sha3: "return sha3(\"hello\")",
            func_ripemd160: "return ripemd160(\"hello\")",
            func_hac_to_mei: "return hac_to_mei(100)",
            func_hac_to_zhu: "return hac_to_zhu(100)",
            func_mei_to_hac: "return mei_to_hac(100)",
            func_zhu_to_hac: "return zhu_to_hac(100)",
            func_u64_to_fold64: "return u64_to_fold64(12345)",
            func_fold64_to_u64: "return fold64_to_u64(0xABCD)",
            func_pack_asset: "return pack_asset(1, 100)",
            func_address_ptr: "return address_ptr(emqjNS9PscqdBpMtnC3Jfuc4mvZUPYTPS)",
            func_context_address: "return context_address()",
            func_block_height: "return block_height()",
            func_storage_load: "return storage_load(\"key\")",
            func_storage_save: "storage_save(\"key\", \"value\")\nreturn 1",
            func_storage_del: "storage_del(\"key\")\nreturn 1",
            func_storage_rest: "return storage_rest(\"key\")",
            func_storage_rent: "storage_rent(\"key\", 100)\nreturn 1",
            func_memory_put: "memory_put(\"key\", \"value\")\nreturn 1",
            func_memory_get: "return memory_get(\"key\")",
            func_global_put: "global_put(\"key\", \"value\")\nreturn 1",
            func_global_get: "return global_get(\"key\")",
            func_heap_grow: "heap_grow(10)\nreturn 1",
            func_heap_write: "heap_grow(10)\nheap_write(0, 0xABCD)\nreturn 1",
            func_heap_read: "heap_grow(10)\nheap_write(0, 0xABCD)\nreturn heap_read(0, 1)",
            func_length: "return length(list { 1 2 3 })",
            func_head: "return head(list { 1 2 3 })",
            func_back: "return back(list { 1 2 3 })",
            func_append: "return append(list { 1 2 }, 3)",
            func_insert: "return insert(list { 1 3 }, 1, 2)",
            func_remove: "return remove(list { 1 2 3 }, 0)",
            func_clone: "return clone(list { 1 2 })",
            func_clear: "var l = list { 1 2 }\nclear(l)\nreturn l",
            func_keys: "return keys(map { \"a\": 1, \"b\": 2 })",
            func_values: "return values(map { \"a\": 1, \"b\": 2 })",
            func_has_key_true: "return has_key(map { \"a\": 1 }, \"a\")",
            func_has_key_false: "return has_key(map { \"a\": 1 }, \"b\")",
            func_buf_cut: "return buf_cut(\"hello world\", 1, 5)",
            func_buf_left: "return buf_left(5, \"hello world\")",
            func_buf_right: "return buf_right(5, \"hello world\")",
            func_buf_left_drop: "return buf_left_drop(5, \"hello world\")",
            func_buf_right_drop: "return buf_right_drop(5, \"hello world\")",
            func_byte: "return byte(\"hello\", 0)",
            func_size: "return size(\"hello\")",
            func_check_signature: "return check_signature(emqjNS9PscqdBpMtnC3Jfuc4mvZUPYTPS)",
            func_balance: "return balance(emqjNS9PscqdBpMtnC3Jfuc4mvZUPYTPS)",
            func_nested_1: "return length(append(list { 1 2 }, 3))",
            func_nested_2: "return size(buf_left_drop(3, buf_right(5, \"hello world\")))",
            func_nested_3: "return head(append(list { 1 }, head(list { 2 3 })))",
        );
    }

    // ========================================================================= 13. FUNCTION CALLS =========================================================================

    mod function_calls {
        use super::*;

        assert_compile_ok!(
            call_this: "return this.func(1)",
            call_self: "return self.func(1)",
            call_super: "return super.func(1)",
        );
    }

    // ========================================================================= 14. COMPLEX SCENARIOS =========================================================================

    mod complex_scenarios {
        use super::*;

        assert_compile_ok!(
            transfer_pattern: r#"
                param { to amount }
                bind bk = "b_" ++ to
                var bal = storage_load(bk)
                if bal is nil {
                    bal = 0 as u64
                }
                storage_save(bk, bal + amount)
                return 1
            "#,
            complex_calc: r#"
                var a = 1 + 2 * 3
                var b = (a - 1) / 2
                var c = a ** 2 + b ** 2
                return c
            "#,
            data_pipeline: r#"
                var data = "hello world"
                var hashed = sha3(data)
                var part = buf_left(8, hashed)
                return size(part)
            "#,
            list_manipulation: r#"
                var lst = list { 1 2 3 }
                lst = append(lst, 4)
                lst = insert(lst, 0, 0)
                var removed = remove(lst, 2)
                return length(lst)
            "#,
            conditional_logic: r#"
                var x = 10
                var result = if x > 0 {
                    if x > 5 { "large" } else { "medium" }
                } else {
                    "negative"
                }
                return result
            "#,
            loop_with_break: r#"
                var i = 0
                var found = 0
                while i < 100 {
                    if i == 50 {
                        found = i
                        break
                    }
                    i += 1
                }
                return found
            "#,
        );
    }

    // ========================================================================= 15. ERROR CASES =========================================================================

    mod error_cases {
        use super::*;

        assert_compile_err!(
            err_unclosed_paren: "return (1 + 2",
            err_unclosed_brace: "if true { return 1",
            err_unclosed_bracket: "return [1, 2",
            err_invalid_op: "return 1 +",
            err_invalid_assign: "1 = 2",
            err_param_empty: "param { }",
        );
    }

    // ========================================================================= 16. BYTECODE GENERATION =========================================================================

    mod bytecode_generation {
        use super::*;

        #[test]
        fn bytecode_int() {
            let result = lang_to_bytecode("return 123");
            assert!(result.is_ok(), "Failed: {:?}", result.err());
            let bytes = result.unwrap();
            assert!(!bytes.is_empty(), "Bytecode should not be empty");
        }

        #[test]
        fn bytecode_string() {
            let result = lang_to_bytecode("return \"hello\"");
            assert!(result.is_ok(), "Failed: {:?}", result.err());
        }

        #[test]
        fn bytecode_expression() {
            let result = lang_to_bytecode("return 1 + 2 * 3");
            assert!(result.is_ok(), "Failed: {:?}", result.err());
        }

        #[test]
        fn bytecode_if_else() {
            let result = lang_to_bytecode("if true { return 1 } else { return 0 }");
            assert!(result.is_ok(), "Failed: {:?}", result.err());
        }

        #[test]
        fn bytecode_while() {
            let result = lang_to_bytecode("var i = 0\nwhile i < 10 { i += 1 }\nreturn i");
            assert!(result.is_ok(), "Failed: {:?}", result.err());
        }

        #[test]
        fn bytecode_function_call() {
            let result = lang_to_bytecode("return length(list { 1 2 3 })");
            assert!(result.is_ok(), "Failed: {:?}", result.err());
        }
    }

    // ========================================================================= 17. BREAK AND CONTINUE RUNTIME TESTS =========================================================================

    mod break_continue_runtime {
        use super::*;
        use crate::lang::lang_to_bytecode;

        fn execute_and_get_value(script: &str) -> u64 {
            use crate::machine::CtxHost;
            use crate::rt::{ExecMode, GasExtra, GasTable, SpaceCap};
            use crate::space::{CtcKVMap, GKVMap, Heap, Stack};
            use crate::value::Value;
            use basis::component::Env;
            use field::Address;
            use protocol::context::ContextInst;
            use protocol::state::EmptyLogs;
            use std::collections::HashMap;

            #[derive(Default, Clone, Debug)]
            struct DummyTx;
            impl field::Serialize for DummyTx {
                fn size(&self) -> usize {
                    0
                }
                fn serialize(&self) -> Vec<u8> {
                    vec![]
                }
            }
            impl basis::interface::TxExec for DummyTx {}
            impl basis::interface::TransactionRead for DummyTx {
                fn ty(&self) -> u8 {
                    3
                }
                fn hash(&self) -> field::Hash {
                    field::Hash::default()
                }
                fn hash_with_fee(&self) -> field::Hash {
                    field::Hash::default()
                }
                fn main(&self) -> Address {
                    Address::default()
                }
                fn addrs(&self) -> Vec<Address> {
                    vec![Address::default()]
                }
                fn fee(&self) -> &field::Amount {
                    field::Amount::zero_ref()
                }
                fn fee_purity(&self) -> u64 {
                    1
                }
                fn fee_extend(&self) -> sys::Ret<u8> {
                    Ok(1)
                }
            }

            #[derive(Default)]
            struct StateMem {
                mem: HashMap<Vec<u8>, Vec<u8>>,
            }
            impl basis::interface::State for StateMem {
                fn get(&self, k: Vec<u8>) -> Option<Vec<u8>> {
                    self.mem.get(&k).cloned()
                }
                fn set(&mut self, k: Vec<u8>, v: Vec<u8>) {
                    self.mem.insert(k, v);
                }
                fn del(&mut self, k: Vec<u8>) {
                    self.mem.remove(&k);
                }
            }

            let codes = lang_to_bytecode(script).expect("Failed to compile");
            let mut pc = 0usize;
            let mut gas: i64 = 65535;
            let cadr = crate::ContractAddress::default();

            let tx = DummyTx::default();
            let mut env = Env::default();
            env.block.height = 1;
            let mut ctx = ContextInst::new(
                env,
                Box::new(StateMem::default()),
                Box::new(EmptyLogs {}),
                &tx,
            );
            let ctx: &mut dyn basis::interface::Context = &mut ctx;

            let mut ops = Stack::new(256);
            let mut heap = Heap::new(64);
            let mut host = CtxHost::new(ctx);

            crate::interpreter::execute_code(
                &mut pc,
                &codes,
                ExecMode::Main,
                false,
                0,
                &mut gas,
                &GasTable::new(1),
                &GasExtra::new(1),
                &SpaceCap::new(1),
                &mut ops,
                &mut Stack::new(256),
                &mut heap,
                &mut GKVMap::new(20),
                &mut CtcKVMap::new(12),
                &mut host,
                &cadr,
                &cadr,
            )
            .expect("Execution failed");

            let result = ops.release().into_iter().last().expect("No return value");
            match result {
                Value::U8(n) => n as u64,
                Value::U16(n) => n as u64,
                Value::U32(n) => n as u64,
                Value::U64(n) => n,
                Value::U128(n) => n as u64,
                _ => panic!("Unexpected return type: {:?}", result),
            }
        }

        #[test]
        fn test_break_exits_loop() {
            let result = execute_and_get_value(
                r#"
                var i = 0
                while i < 10 {
                    if i == 5 { break }
                    i += 1
                }
                return i
            "#,
            );
            assert_eq!(result, 5);
        }

        #[test]
        fn test_break_early() {
            let result = execute_and_get_value(
                r#"
                var i = 0
                while i < 10 {
                    break
                }
                return i
            "#,
            );
            assert_eq!(result, 0);
        }

        #[test]
        fn test_continue_skips_following_statements() {
            let result = execute_and_get_value(
                r#"
                var i = 0
                var touched = 0
                while i < 10 {
                    i += 1
                    if i == 5 { continue }
                    touched += 1
                }
                return touched
            "#,
            );
            assert_eq!(result, 9);
        }

        #[test]
        fn test_continue_sum() {
            let result = execute_and_get_value(
                r#"
                var sum = 0
                var i = 0
                while i < 10 {
                    i += 1
                    if i == 5 { continue }
                    sum += i
                }
                return sum
            "#,
            );
            assert_eq!(result, 50);
        }

        #[test]
        fn test_nested_break() {
            let result = execute_and_get_value(
                r#"
                var i = 0
                var sum = 0
                while i < 5 {
                    i += 1
                    var j = 0
                    while j < 5 {
                        j += 1
                        if j == 3 { break }
                        sum += 1
                    }
                }
                return sum
            "#,
            );
            assert_eq!(result, 10);
        }

        #[test]
        fn test_nested_continue() {
            let result = execute_and_get_value(
                r#"
                var i = 0
                var sum = 0
                while i < 3 {
                    i += 1
                    var j = 0
                    while j < 3 {
                        j += 1
                        if j == 2 { continue }
                        sum += 1
                    }
                }
                return sum
            "#,
            );
            assert_eq!(result, 6);
        }

        #[test]
        fn test_break_continue_combo() {
            let result = execute_and_get_value(
                r#"
                var i = 0
                var sum = 0
                while i < 20 {
                    i += 1
                    if i == 3 { continue }
                    if i > 8 { break }
                    sum += i
                }
                return sum
            "#,
            );
            assert_eq!(result, 33);
        }

        #[test]
        fn test_while_if_continue_break() {
            let result = execute_and_get_value(
                r#"
                var sum = 0
                var i = 0
                while i < 10 {
                    i += 1
                    if i < 5 {
                        continue
                    }
                    if i == 8 {
                        break
                    }
                    sum += i
                }
                return sum
            "#,
            );
            assert_eq!(result, 18);
        }

        #[test]
        fn test_triple_nested_break_only_innermost() {
            let result = execute_and_get_value(
                r#"
                var i = 0
                var score = 0
                while i < 3 {
                    i += 1
                    var j = 0
                    while j < 3 {
                        j += 1
                        var k = 0
                        while k < 5 {
                            k += 1
                            if k == 2 { break }
                            score += 1
                        }
                        score += 10
                    }
                }
                return score
            "#,
            );
            assert_eq!(result, 99);
        }

        #[test]
        fn test_triple_nested_continue_only_innermost() {
            let result = execute_and_get_value(
                r#"
                var i = 0
                var score = 0
                while i < 2 {
                    i += 1
                    var j = 0
                    while j < 3 {
                        j += 1
                        var k = 0
                        while k < 4 {
                            k += 1
                            if k == 2 { continue }
                            score += 1
                        }
                    }
                }
                return score
            "#,
            );
            assert_eq!(result, 18);
        }

        #[test]
        fn test_inner_break_with_outer_continue() {
            let result = execute_and_get_value(
                r#"
                var i = 0
                var score = 0
                while i < 5 {
                    i += 1
                    var j = 0
                    while j < 5 {
                        j += 1
                        if j == 3 { break }
                        score += 1
                    }
                    if i % 2 == 0 { continue }
                    score += 10
                }
                return score
            "#,
            );
            assert_eq!(result, 40);
        }

        #[test]
        fn test_unreachable_break_continue_in_while_false() {
            let result = execute_and_get_value(
                r#"
                var x = 7
                while false {
                    x = 100
                    break
                }
                while false {
                    x = 200
                    continue
                }
                return x
            "#,
            );
            assert_eq!(result, 7);
        }
    }
}
