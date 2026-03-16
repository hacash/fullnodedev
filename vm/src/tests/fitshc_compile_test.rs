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
    fn rejects_missing_arg_colon() {
        let src = r#"
            contract demo {
                function external f(a u8) -> u8 { return 1 }
            }
        "#;
        expect_compile_err(src, "expected ':' after arg name");
    }

    #[test]
    fn rejects_non_contract_library_address_without_panic() {
        let src = r#"
            contract demo {
                library [A: 18dekVcACnj6Tbd69SsexVMQ5KLBZZfn5K]
                function external f() -> u8 { return 1 }
            }
        "#;
        let res = std::panic::catch_unwind(|| fitshc_compile(src));
        assert!(res.is_ok(), "fitshc compile panicked for non-contract library address");
        let err = match res.unwrap() {
            Ok(_) => panic!("fitshc compile should fail"),
            Err(e) => e,
        };
        assert!(err.to_string().contains("is not CONTRACT"));
    }

    #[test]
    fn rejects_non_contract_inherit_address_without_panic() {
        let src = r#"
            contract demo {
                inherit [A: 18dekVcACnj6Tbd69SsexVMQ5KLBZZfn5K]
                function external f() -> u8 { return 1 }
            }
        "#;
        let res = std::panic::catch_unwind(|| fitshc_compile(src));
        assert!(res.is_ok(), "fitshc compile panicked for non-contract inherit address");
        let err = match res.unwrap() {
            Ok(_) => panic!("fitshc compile should fail"),
            Err(e) => e,
        };
        assert!(err.to_string().contains("is not CONTRACT"));
    }

    #[test]
    fn rejects_duplicate_top_level_const_without_body_compile() {
        let src = r#"
            contract demo {
                const A = 1
                const A = 2
            }
        "#;
        expect_compile_err(src, "duplicate const 'A'");
    }

    #[test]
    fn rejects_inherit_count_overflow_without_panic() {
        use field::{Address, Uint4};
        use crate::ContractAddress;

        let base = Address::from_readable("18dekVcACnj6Tbd69SsexVMQ5KLBZZfn5K").unwrap();
        let inherit = (0..=u8::MAX as u32)
            .map(|idx| {
                let addr = ContractAddress::calculate(&base, &Uint4::from(idx));
                format!("I{}: {}", idx, addr.to_addr().to_readable())
            })
            .collect::<Vec<_>>()
            .join(", ");
        let src = format!(
            "contract demo {{
    inherit [{}]
    function external f() -> u8 {{ return 1 }}
}}",
            inherit
        );
        let res = std::panic::catch_unwind(|| fitshc_compile(&src));
        assert!(res.is_ok(), "fitshc compile panicked for overflowing inherit count");
        let err = match res.unwrap() {
            Ok(_) => panic!("fitshc compile should fail"),
            Err(e) => e,
        };
        assert!(err.to_string().contains("too many inherit contracts: max 255"));
    }

    #[test]
    fn rejects_duplicate_library_address() {
        let src = r#"
            contract demo {
                library [A: emqjNS9PscqdBpMtnC3Jfuc4mvZUPYTPS, B: emqjNS9PscqdBpMtnC3Jfuc4mvZUPYTPS]
                function external f() -> u8 { return 1 }
            }
        "#;
        expect_compile_err(src, "duplicate library address");
    }

    #[test]
    fn rejects_duplicate_inherit_address() {
        let src = r#"
            contract demo {
                inherit [A: emqjNS9PscqdBpMtnC3Jfuc4mvZUPYTPS, B: emqjNS9PscqdBpMtnC3Jfuc4mvZUPYTPS]
                function external f() -> u8 { return 1 }
            }
        "#;
        expect_compile_err(src, "duplicate inherit address");
    }

    #[test]
    fn rejects_duplicate_function_signature() {
        let src = r#"
            contract demo {
                function external f() -> u8 { return 1 }
                function external f() -> u8 { return 2 }
            }
        "#;
        expect_compile_err(src, "duplicate function 'f' signature");
    }

    #[test]
    fn rejects_duplicate_abstract_signature() {
        let src = r#"
            contract demo {
                abstract Construct(data: bytes) { abort }
                abstract Construct(data: bytes) { abort }
            }
        "#;
        expect_compile_err(src, "duplicate abstract 'Construct'");
    }

    #[test]
    fn rejects_contract_library_count_overflow_without_panic() {
        use field::{Address, Uint4};
        use crate::ContractAddress;

        let base = Address::from_readable("18dekVcACnj6Tbd69SsexVMQ5KLBZZfn5K").unwrap();
        let libs = (0..=u8::MAX as u32)
            .map(|idx| {
                let addr = ContractAddress::calculate(&base, &Uint4::from(idx));
                format!("L{}: {}", idx, addr.to_addr().to_readable())
            })
            .collect::<Vec<_>>()
            .join(", ");
        let src = format!(
            "contract demo {{
    library [{}]
    function external f() -> u8 {{ return 1 }}
}}",
            libs
        );
        let res = std::panic::catch_unwind(|| fitshc_compile(&src));
        assert!(res.is_ok(), "fitshc compile panicked for overflowing library count");
        let err = match res.unwrap() {
            Ok(_) => panic!("fitshc compile should fail"),
            Err(e) => e,
        };
        assert!(err.to_string().contains("too many contract libraries: max 255"));
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
    fn accepts_tuple_return_type_and_constructor() {
        let src = r#"
            contract demo {
                function build() -> tuple {
                    return tuple(7, map { "kind": "hnft" })
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
