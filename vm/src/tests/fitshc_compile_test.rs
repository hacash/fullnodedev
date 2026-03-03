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
                function public f() -> u8 { return 1 }
            }
        "#;
        expect_compile_err(src, "unexpected token in contract body");
    }

    #[test]
    fn accepts_use_pragma_prefix_before_contract() {
        let src = r#"
            use pragma 0.1.0
            contract demo {
                function public f() -> u8 { return 1 }
            }
        "#;
        assert!(fitshc_compile(src).is_ok(), "fitshc compile should succeed");
    }

    #[test]
    fn rejects_trailing_tokens_after_contract_end() {
        let src = r#"
            contract demo {
                function public f() -> u8 { return 1 }
            }
            function public g() -> u8 { return 2 }
        "#;
        expect_compile_err(src, "unexpected token after contract end");
    }

    #[test]
    fn rejects_unclosed_parenthesized_return_type() {
        let src = r#"
            contract demo {
                function public f() -> (u8 { return 1 }
            }
        "#;
        expect_compile_err(src, "expected ')' after return type");
    }
}
