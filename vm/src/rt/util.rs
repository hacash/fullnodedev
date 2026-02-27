
pub fn local_operand_param_parse(mark: u8) -> (String, u8) {
    let opt = mark >> 6; // 0b00000011
    let idx = mark & 0b00111111; // max=64
    (match opt {
        0 => "+=",
        1 => "-=",
        2 => "*=",
        3 => "/=",
        _ => unreachable!()
    }.to_owned(), idx)
}


pub fn local_logic_param_parse(mark: u8) -> (String, u8) {
    let opt = mark >> 5; // 0b00000111
    let idx = mark & 0b00011111; // max=32
    (match opt {
        0 => "&&",
        1 => "||",
        2 => "==",
        3 => "!=",
        4 => ">",
        5 => ">=",
        6 => "<",
        7 => "<=",
        _ => unreachable!()
    }.to_owned(), idx)
}


pub fn ascii_show_string(s: &[u8]) -> Option<String> {
    maybe!(s.iter().any(|&a|a!=10&&(a<32||a>126)),
        None,
        Some(String::from_utf8(s.to_vec()).unwrap())
    )
}

#[cfg(test)]
mod util_tests {
    use super::*;

    #[test]
    fn local_logic_param_parse_ordering_symbols_match_display_order() {
        let (op4, idx4) = local_logic_param_parse((4 << 5) | 3);
        assert_eq!(op4, ">");
        assert_eq!(idx4, 3);

        let (op5, idx5) = local_logic_param_parse((5 << 5) | 7);
        assert_eq!(op5, ">=");
        assert_eq!(idx5, 7);

        let (op6, idx6) = local_logic_param_parse((6 << 5) | 1);
        assert_eq!(op6, "<");
        assert_eq!(idx6, 1);

        let (op7, idx7) = local_logic_param_parse((7 << 5) | 31);
        assert_eq!(op7, "<=");
        assert_eq!(idx7, 31);
    }
}
