use vm::exec_test::execute_lang_with_params;

#[test]
fn choose_returns_yes_when_cond_true() {
    // Use numeric branches to avoid ambiguity of literal typing in tests.
    let src = "let r = choose(true, 1, 0); return r";
    let res = execute_lang_with_params(src, "").expect("execution failed");
    // Accept any numeric type and compare via to_uint()
    assert_eq!(res.to_uint(), 1, "expected chosen value 1");
}

#[test]
fn choose_returns_no_when_cond_false() {
    let src = "let r = choose(false, 1, 0); return r";
    let res = execute_lang_with_params(src, "").expect("execution failed");
    assert_eq!(res.to_uint(), 0, "expected chosen value 0");
}
