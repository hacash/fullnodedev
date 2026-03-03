
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum LxOp {
    Add,
    Sub,
    Mul,
    Div,
}

impl LxOp {
    fn bits(self) -> u8 {
        match self {
            LxOp::Add => 0,
            LxOp::Sub => 1,
            LxOp::Mul => 2,
            LxOp::Div => 3,
        }
    }

    pub fn symbol(self) -> &'static str {
        match self {
            LxOp::Add => "+=",
            LxOp::Sub => "-=",
            LxOp::Mul => "*=",
            LxOp::Div => "/=",
        }
    }

    fn from_bits(opt: u8) -> Self {
        match opt {
            0 => LxOp::Add,
            1 => LxOp::Sub,
            2 => LxOp::Mul,
            3 => LxOp::Div,
            _ => unreachable!(),
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum LxLg {
    And,
    Or,
    Eq,
    Ne,
    Gt,
    Ge,
    Lt,
    Le,
}

impl LxLg {
    fn bits(self) -> u8 {
        match self {
            LxLg::And => 0,
            LxLg::Or => 1,
            LxLg::Eq => 2,
            LxLg::Ne => 3,
            LxLg::Gt => 4,
            LxLg::Ge => 5,
            LxLg::Lt => 6,
            LxLg::Le => 7,
        }
    }

    pub fn symbol(self) -> &'static str {
        match self {
            LxLg::And => "&&",
            LxLg::Or => "||",
            LxLg::Eq => "==",
            LxLg::Ne => "!=",
            LxLg::Gt => ">",
            LxLg::Ge => ">=",
            LxLg::Lt => "<",
            LxLg::Le => "<=",
        }
    }

    fn from_bits(opt: u8) -> Self {
        match opt {
            0 => LxLg::And,
            1 => LxLg::Or,
            2 => LxLg::Eq,
            3 => LxLg::Ne,
            4 => LxLg::Gt,
            5 => LxLg::Ge,
            6 => LxLg::Lt,
            7 => LxLg::Le,
            _ => unreachable!(),
        }
    }
}

pub const LXOP_MAX_IDX: u8 = 0b_0011_1111;
pub const LXLG_MAX_IDX: u8 = 0b_0001_1111;

pub fn encode_local_operand_mark(op: LxOp, idx: u8) -> VmrtRes<u8> {
    if idx > LXOP_MAX_IDX {
        return Err(ItrErr::new(
            ItrErrCode::InstParamsErr,
            &format!("local operand idx {} out of range {}", idx, LXOP_MAX_IDX),
        ))
    }
    Ok((op.bits() << 6) | idx)
}

pub fn encode_local_logic_mark(op: LxLg, idx: u8) -> VmrtRes<u8> {
    if idx > LXLG_MAX_IDX {
        return Err(ItrErr::new(
            ItrErrCode::InstParamsErr,
            &format!("local logic idx {} out of range {}", idx, LXLG_MAX_IDX),
        ))
    }
    Ok((op.bits() << 5) | idx)
}

pub fn decode_local_operand_mark(mark: u8) -> (LxOp, u8) {
    let opt = mark >> 6; // high 2 bits
    let idx = mark & LXOP_MAX_IDX; // low 6 bits, max=64
    (LxOp::from_bits(opt), idx)
}

pub fn decode_local_logic_mark(mark: u8) -> (LxLg, u8) {
    let opt = mark >> 5; // high 3 bits
    let idx = mark & LXLG_MAX_IDX; // low 5 bits, max=32
    (LxLg::from_bits(opt), idx)
}

pub fn local_operand_param_parse(mark: u8) -> (String, u8) {
    let (op, idx) = decode_local_operand_mark(mark);
    (op.symbol().to_owned(), idx)
}

pub fn local_logic_param_parse(mark: u8) -> (String, u8) {
    let (op, idx) = decode_local_logic_mark(mark);
    (op.symbol().to_owned(), idx)
}

#[cfg(test)]
mod util_tests {
    use super::*;

    #[test]
    fn decode_local_operand_mark_matches_symbol_and_index() {
        let (op0, idx0) = decode_local_operand_mark(encode_local_operand_mark(LxOp::Add, 9).unwrap());
        assert_eq!(op0, LxOp::Add);
        assert_eq!(op0.symbol(), "+=");
        assert_eq!(idx0, 9);

        let (op1, idx1) = decode_local_operand_mark(encode_local_operand_mark(LxOp::Sub, 8).unwrap());
        assert_eq!(op1, LxOp::Sub);
        assert_eq!(op1.symbol(), "-=");
        assert_eq!(idx1, 8);

        let (op2, idx2) = decode_local_operand_mark(encode_local_operand_mark(LxOp::Mul, 7).unwrap());
        assert_eq!(op2, LxOp::Mul);
        assert_eq!(op2.symbol(), "*=");
        assert_eq!(idx2, 7);

        let (op3, idx3) = decode_local_operand_mark(encode_local_operand_mark(LxOp::Div, 6).unwrap());
        assert_eq!(op3, LxOp::Div);
        assert_eq!(op3.symbol(), "/=");
        assert_eq!(idx3, 6);
    }

    #[test]
    fn local_logic_param_parse_ordering_symbols_match_display_order() {
        let (kind4, idx4d) = decode_local_logic_mark(encode_local_logic_mark(LxLg::Gt, 3).unwrap());
        assert_eq!(kind4, LxLg::Gt);
        assert_eq!(kind4.symbol(), ">");
        assert_eq!(idx4d, 3);
        let (op4, idx4) = local_logic_param_parse(encode_local_logic_mark(LxLg::Gt, 3).unwrap());
        assert_eq!(op4, ">");
        assert_eq!(idx4, 3);

        let (kind5, idx5d) = decode_local_logic_mark(encode_local_logic_mark(LxLg::Ge, 7).unwrap());
        assert_eq!(kind5, LxLg::Ge);
        assert_eq!(kind5.symbol(), ">=");
        assert_eq!(idx5d, 7);
        let (op5, idx5) = local_logic_param_parse(encode_local_logic_mark(LxLg::Ge, 7).unwrap());
        assert_eq!(op5, ">=");
        assert_eq!(idx5, 7);

        let (kind6, idx6d) = decode_local_logic_mark(encode_local_logic_mark(LxLg::Lt, 1).unwrap());
        assert_eq!(kind6, LxLg::Lt);
        assert_eq!(kind6.symbol(), "<");
        assert_eq!(idx6d, 1);
        let (op6, idx6) = local_logic_param_parse(encode_local_logic_mark(LxLg::Lt, 1).unwrap());
        assert_eq!(op6, "<");
        assert_eq!(idx6, 1);

        let (kind7, idx7d) = decode_local_logic_mark(encode_local_logic_mark(LxLg::Le, 31).unwrap());
        assert_eq!(kind7, LxLg::Le);
        assert_eq!(kind7.symbol(), "<=");
        assert_eq!(idx7d, 31);
        let (op7, idx7) = local_logic_param_parse(encode_local_logic_mark(LxLg::Le, 31).unwrap());
        assert_eq!(op7, "<=");
        assert_eq!(idx7, 31);
    }

    #[test]
    fn encode_mark_rejects_out_of_range_index() {
        assert!(matches!(
            encode_local_operand_mark(LxOp::Add, 64),
            Err(ItrErr(ItrErrCode::InstParamsErr, _))
        ));
        assert!(matches!(
            encode_local_logic_mark(LxLg::Eq, 32),
            Err(ItrErr(ItrErrCode::InstParamsErr, _))
        ));
    }

}
