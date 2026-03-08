#[cfg(test)]
mod fitshc_compile_tests {
    use crate::fitshc::compile as fitshc_compile;

    fn expect_compile_err(src: &str, needle: &str) {
        let err = match fitshc_compile(src) {
            Ok(_) => panic!("fitshc compile should fail"),
            Err(e) => e,
        };
        let text = err.to_string();
        assert!(
            text.contains(needle),
            "error mismatch\nsource:\n{}\nexpected substring: {}\nactual: {}",
            src,
            needle,
            text
        );
    }

    #[test]
    fn rejects_unknown_contract_body_token_instead_of_skipping() {
        let src = r#"
            contract demo {
                return 1
                function external f() -> u8 { return 1 }
            }
        "#;
        expect_compile_err(src, "unexpected token in contract body");
    }

    #[test]
    fn accepts_use_pragma_prefix_before_contract() {
        let src = r#"
            use pragma 0.1.0
            contract demo {
                function external f() -> u8 { return 1 }
            }
        "#;
        assert!(fitshc_compile(src).is_ok(), "fitshc compile should succeed");
    }

    #[test]
    fn rejects_trailing_tokens_after_contract_end() {
        let src = r#"
            contract demo {
                function external f() -> u8 { return 1 }
            }
            function external g() -> u8 { return 2 }
        "#;
        expect_compile_err(src, "unexpected token after contract end");
    }

    #[test]
    fn rejects_unclosed_parenthesized_return_type() {
        let src = r#"
            contract demo {
                function external f() -> (u8 { return 1 }
            }
        "#;
        expect_compile_err(src, "expected ')' after return type");
    }

    #[test]
    fn accepts_compo_param_and_return_types() {
        let src = r#"
            contract demo {
                function helper(doc: map) -> map {
                    return doc
                }
                function external run() -> map {
                    return this.helper(map { "a": 1 })
                }
            }
        "#;
        assert!(fitshc_compile(src).is_ok(), "fitshc compile should succeed");
    }


    #[test]
    fn accepts_args_return_type_and_constructor() {
        let src = r#"
            contract demo {
                function build() -> args {
                    return args(7, map { "kind": "hnft" })
                }
                function consume(num: u8, doc: map) -> u8 {
                    assert doc is map
                    return num
                }
                function external run() -> u8 {
                    return this.consume(this.build())
                }
            }
        "#;
        assert!(fitshc_compile(src).is_ok(), "fitshc compile should succeed");
    }
}
