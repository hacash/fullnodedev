
use protocol::action::*;
use super::action::*;
use ValueTy::*;

pub type ExtDefTy = (u8, &'static str, ValueTy, usize);

const CALL_EXTEND_UNKNOWN_NAME: &'static str = "__unknown__";


/********************************************/


pub const CALL_EXTEND_ACTION_DEFS: [ExtDefTy; 4] = [
    (HacToTrs::IDX,   "transfer_hac_to",      Nil,      2),
    (SatToTrs::IDX,   "transfer_sat_to",      Nil,      2),
    (HacFromTrs::IDX, "transfer_hac_from",    Nil,      2),
    (SatFromTrs::IDX, "transfer_sat_from",    Nil,      2),
];


pub const CALL_EXTEND_ENV_DEFS: [ExtDefTy; 2] = [
    (EnvHeight::IDX,   "block_height",             U64,  0),
    (EnvMainAddr::IDX, "tx_main_address", ValueTy::Address,  0),
];


pub const CALL_EXTEND_FUNC_DEFS: [ExtDefTy; 2] = [
    (FuncCheckSign::IDX, "check_signature",         Bool,  1),
    (FuncBalance::IDX,   "balance",                 Bytes, 1),
];


/********************************************/


pub fn search_ext_by_id<'a>(id: u8, exts: &'a[ExtDefTy]) -> Option<&'a ExtDefTy> {
    for a in exts {
        if a.0 == id {
            return Some(a)
        }
    }
    // not find
    None
}

pub fn search_ext_name_by_id(id: u8, exts: &[ExtDefTy]) -> &'static str {
     match search_ext_by_id(id, exts) {
        Some(a) => a.1,
        _ => CALL_EXTEND_UNKNOWN_NAME
    }
}