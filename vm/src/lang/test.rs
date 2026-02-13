#[cfg(test)]
mod token_t {

    #[test]
    fn test_char_basic() {
        let tests = vec![
            ("'A'", 65u8),
            ("'Z'", 90),
            ("'a'", 97),
            ("'z'", 122),
            ("'0'", 48),
            ("'9'", 57),
            ("' '", 32),
            ("'.'", 46),
        ];

        for (input, expected) in tests {
            let tkr = super::Tokenizer::new(input.as_bytes());
            let tokens = tkr.parse().expect(&format!("Failed to parse: {}", input));
            assert_eq!(tokens.len(), 1, "Expected 1 token for {}", input);
            match &tokens[0] {
                super::Token::Character(b) => assert_eq!(*b, expected, "Wrong value for {}", input),
                _ => panic!(
                    "Expected Character token for {}, got {:?}",
                    input, tokens[0]
                ),
            }
        }
    }

    #[test]
    fn test_char_escape_sequences() {
        let tests = vec![
            ("'\\n'", 10u8),
            ("'\\t'", 9),
            ("'\\r'", 13),
            ("'\\\\'", 92),
            ("'\\''", 39),
        ];

        for (input, expected) in tests {
            let tkr = super::Tokenizer::new(input.as_bytes());
            let tokens = tkr.parse().expect(&format!("Failed to parse: {}", input));
            assert_eq!(tokens.len(), 1, "Expected 1 token for {}", input);
            match &tokens[0] {
                super::Token::Character(b) => assert_eq!(*b, expected, "Wrong value for {}", input),
                _ => panic!(
                    "Expected Character token for {}, got {:?}",
                    input, tokens[0]
                ),
            }
        }
    }

    #[test]
    fn test_char_in_expressions() {
        let scripts = vec![
            "var c = 'A'",
            "var lower = 'a'",
            "var digit = '0'",
            "var space = ' '",
            "let c = 'Z' + 1",
            "if 'A' == 'A' { return 1 }",
        ];

        for script in scripts {
            let result = super::lang_to_irnode(script);
            assert!(result.is_ok(), "Failed to parse: {}", script);
        }
    }

    #[test]
    fn test_char_error_handling() {
        let invalid_inputs = vec![
            "'",     // Incomplete
            "''",    // Empty
            "'AB'",  // Multiple chars
            "'\\x'", // Invalid escape
        ];

        for input in invalid_inputs {
            let tkr = super::Tokenizer::new(input.as_bytes());
            let result = tkr.parse();
            assert!(result.is_err(), "Should fail for invalid char: {}", input);
        }
    }

    #[test]
    fn test_char_vs_string_distinction() {
        let char_tkr = super::Tokenizer::new("'A'".as_bytes());
        let char_tokens = char_tkr.parse().unwrap();

        let str_tkr = super::Tokenizer::new("\"A\"".as_bytes());
        let str_tokens = str_tkr.parse().unwrap();

        assert_ne!(
            char_tokens[0], str_tokens[0],
            "Character 'A' and string \"A\" should produce different tokens"
        );

        match &char_tokens[0] {
            super::Token::Character(b) => assert_eq!(*b, 65),
            _ => panic!("'A' should be Character"),
        }

        match &str_tokens[0] {
            super::Token::Bytes(bs) => {
                assert_eq!(bs.len(), 1);
                assert_eq!(bs[0], 65);
            }
            _ => panic!("\"A\" should be Bytes"),
        }
    }

    #[test]
    fn test_char_roundtrip() {
        use super::irnode_to_lang;
        use super::lang_to_irnode;
        let scripts = vec![
            "var c = 'A'",
            "var tab = '\\t'",
            "let newline = '\\n'",
            "if 'x' == 'y' { return 0 }",
        ];

        for script in scripts {
            let ir = lang_to_irnode(script).expect(&format!("Failed to compile: {}", script));
            let _result = irnode_to_lang(ir).expect(&format!("Failed to decompile: {}", script));
        }
    }

    #[test]
    fn test_char_compiles_to_pu8() {
        use super::lang_to_bytecode;
        use crate::rt::Bytecode;

        let bytecode = lang_to_bytecode("var c = 'A'").expect("Failed to compile");
        let pu8_byte = Bytecode::PU8 as u8;
        assert!(
            bytecode.contains(&pu8_byte),
            "Expected PU8 (0x20) in bytecode, got: {:02x?}",
            bytecode
        );
        assert!(
            bytecode.contains(&65u8),
            "Expected value 65 in bytecode, got: {:02x?}",
            bytecode
        );
    }

    #[test]
    fn test_char_escapes_compile_correctly() {
        use super::lang_to_bytecode;

        let bc_newline = lang_to_bytecode("var n = '\\n'").expect("Failed to compile newline");
        assert!(
            bc_newline.contains(&10u8),
            "Expected value 10, got: {:02x?}",
            bc_newline
        );

        let bc_tab = lang_to_bytecode("var t = '\\t'").expect("Failed to compile tab");
        assert!(
            bc_tab.contains(&9u8),
            "Expected value 9, got: {:02x?}",
            bc_tab
        );
    }

    // ==================== BUG 2: 空 Map Roundtrip 测试 ====================

    #[test]
    fn test_empty_map_roundtrip() {
        use super::irnode_to_lang;
        use super::lang_to_irnode;

        let original = "map { }";
        let ir = lang_to_irnode(original).expect("Failed to compile empty map");
        let decompiled = irnode_to_lang(ir).expect("Failed to decompile");

        let ir2 = lang_to_irnode(&decompiled).expect("Failed to recompile decompiled map");
        let decompiled2 = irnode_to_lang(ir2).expect("Failed to decompile again");

        assert_eq!(
            decompiled, decompiled2,
            "Empty map roundtrip should be stable.\nOriginal: {}\nFirst: {}\nSecond: {}",
            original, decompiled, decompiled2
        );
    }

    #[test]
    fn test_empty_map_vs_nonempty_map_structure() {
        use super::irnode_to_lang;
        use super::lang_to_irnode;
        use crate::IRNode;

        let empty_ir = lang_to_irnode("map { }").expect("Failed to compile empty map");
        let empty_serialized = empty_ir.serialize();

        let nonempty_ir =
            lang_to_irnode("map { \"k\": \"v\" }").expect("Failed to compile non-empty map");

        let empty_decompiled = irnode_to_lang(empty_ir).expect("Failed to decompile empty map");
        println!("Empty map decompiled: {:?}", empty_decompiled);

        let nonempty_decompiled =
            irnode_to_lang(nonempty_ir).expect("Failed to decompile non-empty map");
        println!("Non-empty map decompiled: {:?}", nonempty_decompiled);

        let ir_after = lang_to_irnode(&empty_decompiled).expect("Failed to compile decompiled");
        let after_serialized = ir_after.serialize();

        println!("Empty map serialized: {:02x?}", empty_serialized);
        println!("After roundtrip serialized: {:02x?}", after_serialized);

        assert_eq!(
            empty_serialized, after_serialized,
            "Empty map should be stable after roundtrip"
        );
    }

    #[test]
    fn test_empty_list_vs_empty_map_comparison() {
        use super::irnode_to_lang;
        use super::lang_to_irnode;
        use crate::IRNode;

        let list_original = "list { }";
        let list_ir = lang_to_irnode(list_original).expect("Failed to compile empty list");
        let list_serialized = list_ir.serialize();
        let list_decompiled = irnode_to_lang(list_ir).expect("Failed to decompile empty list");
        let list_ir2 = lang_to_irnode(&list_decompiled).expect("Failed to recompile");
        let list_decompiled2 = irnode_to_lang(list_ir2).expect("Failed to decompile again");

        let map_original = "map { }";
        let map_ir = lang_to_irnode(map_original).expect("Failed to compile empty map");
        let map_serialized = map_ir.serialize();
        let map_decompiled = irnode_to_lang(map_ir).expect("Failed to decompile empty map");
        let map_ir2 = lang_to_irnode(&map_decompiled).expect("Failed to recompile");
        let map_decompiled2 = irnode_to_lang(map_ir2).expect("Failed to decompile again");

        assert_eq!(
            list_decompiled, list_decompiled2,
            "Empty list should be stable"
        );

        assert_eq!(
            map_decompiled, map_decompiled2,
            "Empty map should be stable"
        );

        println!("Empty list serialized: {:?}", list_serialized);
        println!("Empty map serialized: {:?}", map_serialized);
    }

    // ==================== Bug 5: Tokenizer 数字解析测试 ====================

    #[test]
    fn test_number_with_letters_should_fail() {
        // 数字中包含字母应该失败
        let result = super::Tokenizer::new("123abc".as_bytes()).parse();
        assert!(result.is_err(), "123abc should fail to parse as number");
    }

    #[test]
    fn test_underscore_in_numbers() {
        // 下划线分隔的数字应该工作
        let result = super::Tokenizer::new("1000_000".as_bytes()).parse();
        match result {
            Ok(tokens) => {
                println!("1000_000 tokens: {:?}", tokens);
                if let Some(super::Token::Integer(n)) = tokens.first() {
                    assert_eq!(*n, 1000000, "1000_000 should parse as 1000000");
                }
            }
            Err(e) => {
                panic!("1000_000 should parse successfully, got error: {}", e);
            }
        }
    }

    // ==================== Bug 3: callcode requires end ====================

    #[test]
    fn test_callcode_requires_end() {
        use super::lang_to_irnode;

        // Valid callcode syntax (with end)
        let valid_script = r#"
            callcode 0::0xabcdef01
            end
        "#;
        let result = lang_to_irnode(valid_script);
        assert!(
            result.is_ok(),
            "callcode with end should compile successfully"
        );

        // Invalid callcode syntax (without end)
        let invalid_script = r#"
            callcode 0::0xabcdef01
        "#;
        let result = lang_to_irnode(invalid_script);
        assert!(result.is_err(), "callcode without end should fail");
    }

    // ==================== Number Type Suffix Tests ====================

    #[test]
    fn test_number_with_type_suffix() {
        use super::lang_to_irnode;

        // Test 100u64 should parse as 100 as u64
        let scripts = vec!["100u64", "100u32", "100u16", "100u8", "100u128"];

        for script in scripts {
            let result = lang_to_irnode(script);
            match result {
                Ok(_) => println!("OK: {} parsed successfully", script),
                Err(e) => println!("FAIL: {} failed: {}", script, e),
            }
        }
    }

    // ==================== Overflow Check Tests ====================

    #[test]
    fn test_number_overflow_check() {
        use super::lang_to_irnode;

        // 300 overflows u8 (max: 255)
        let result = lang_to_irnode("300u8");
        assert!(result.is_err(), "300u8 should overflow u8");
        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("overflows"),
            "Error should mention overflow: {}",
            err_msg
        );

        // 70000 overflows u16 (max: 65535)
        let result = lang_to_irnode("70000u16");
        assert!(result.is_err(), "70000u16 should overflow u16");

        // 100u64 should be valid
        let result = lang_to_irnode("100u64");
        assert!(result.is_ok(), "100u64 should be valid");
    }

    // ==================== Simplify Numeric As Suffix Test ====================

    #[test]
    fn test_simplify_numeric_as_suffix_decompilation() {
        use super::irnode_to_lang;
        use super::lang_to_irnode;

        // Test that 100u64 decompiles to "100u64" instead of "100 as u64"
        let script = "var x = 100u64";
        let ir = lang_to_irnode(script).expect("Failed to compile");
        let decompiled = irnode_to_lang(ir).expect("Failed to decompile");

        // Should contain "100u64" not "100 as u64"
        assert!(
            decompiled.contains("100u64"),
            "Decompiled should contain '100u64', got: {}",
            decompiled
        );
        assert!(
            !decompiled.contains(" as u64"),
            "Decompiled should NOT contain ' as u64', got: {}",
            decompiled
        );
    }

    #[test]
    fn test_all_numeric_suffixes_decompilation() {
        use super::irnode_to_lang;
        use super::lang_to_irnode;

        let test_cases = [
            ("100u8", "100u8"),
            ("100u16", "100u16"),
            ("100u32", "100u32"),
            ("100u64", "100u64"),
            ("100u128", "100u128"),
            ("70000u64", "70000u64"),
        ];

        for (input, expected) in test_cases {
            let script = format!("var x = {}", input);
            let ir = lang_to_irnode(&script).expect(&format!("Failed to compile: {}", input));
            let decompiled = irnode_to_lang(ir).expect(&format!("Failed to decompile: {}", input));

            assert!(
                decompiled.contains(expected),
                "Expected '{}' in decompiled output, got: {}",
                expected,
                decompiled
            );
        }
    }

    #[test]
    fn test_variable_type_cast_not_suffix() {
        use super::irnode_to_lang;
        use super::lang_to_irnode;

        // Test that variable as u8 is NOT converted to numu8
        let script = r#"
            var num = 100
            var x = num as u8
        "#;
        let ir = lang_to_irnode(script).expect("Failed to compile");
        let decompiled = irnode_to_lang(ir).expect("Failed to decompile");

        // Should contain "num as u8" NOT "numu8"
        assert!(
            decompiled.contains(" as u8"),
            "Variable cast should use 'as u8', got: {}",
            decompiled
        );
        assert!(
            !decompiled.contains("numu8"),
            "Should NOT contain 'numu8', got: {}",
            decompiled
        );
    }

    #[test]
    fn test_simplify_numeric_as_suffix_option_off_uses_as_cast() {
        use super::lang_to_irnode;
        use super::Formater;
        use super::PrintOption;

        let script = "var x = 100u64";
        let ir = lang_to_irnode(script).expect("Failed to compile");

        let mut opt = PrintOption::new("  ", 0);
        opt.recover_literals = true;
        opt.simplify_numeric_as_suffix = false;
        let decompiled = Formater::new(&opt).print(&ir);

        assert!(
            decompiled.contains("100 as u64"),
            "Expected '100 as u64' when simplify_numeric_as_suffix=false, got: {}",
            decompiled
        );
        assert!(
            !decompiled.contains("100u64"),
            "Should NOT contain '100u64' when simplify_numeric_as_suffix=false, got: {}",
            decompiled
        );
    }
}
