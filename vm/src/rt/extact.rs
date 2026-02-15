
use protocol::action::*;
use super::action::*;
use ValueTy::*;

type ExtDefTy = (u8, &'static str, ValueTy, usize);

const CALL_EXTEND_UNKNOWN_NAME: &str = "__unknown__";


/********************************************/


pub const CALL_EXTEND_ACTION_DEFS: [ExtDefTy; 13] = [
    // HAC
    (HacToTrs::IDX,      "transfer_hac_to",         Nil, 2),
    (HacFromTrs::IDX,    "transfer_hac_from",       Nil, 2),
    (HacFromToTrs::IDX,  "transfer_hac_from_to",    Nil, 3),
    // SAT
    (SatToTrs::IDX,      "transfer_sat_to",         Nil, 2),
    (SatFromTrs::IDX,    "transfer_sat_from",       Nil, 2),
    (SatFromToTrs::IDX,  "transfer_sat_from_to",    Nil, 3),
    // HACD
    (DiaSingleTrs::IDX,  "transfer_hacd_single_to", Nil, 2),
    (DiaToTrs::IDX,      "transfer_hacd_to",        Nil, 2),
    (DiaFromTrs::IDX,    "transfer_hacd_from",      Nil, 2),
    (DiaFromToTrs::IDX,  "transfer_hacd_from_to",   Nil, 3),
    // Asset
    (AssetToTrs::IDX,    "transfer_asset_to",       Nil, 2),
    (AssetFromTrs::IDX,  "transfer_asset_from",     Nil, 2),
    (AssetFromToTrs::IDX,"transfer_asset_from_to",  Nil, 3),
];


pub const CALL_EXTEND_ENV_DEFS: [ExtDefTy; 2] = [
    (EnvHeight::IDX,   "block_height",             U64,  0),
    (EnvMainAddr::IDX, "tx_main_address", ValueTy::Address,  0),
];


pub const CALL_EXTEND_VIEW_DEFS: [ExtDefTy; 4] = [
    (FuncCheckSign::IDX,      "check_signature",   Bool,  1),
    (FuncBalance::IDX,        "balance",           Bytes, 1),
    (FuncDiamondInscNum::IDX, "diamond_insc_num",  U8,    1),
    (FuncDiamondInscGet::IDX, "diamond_insc_get",  Bytes, 2),
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

pub fn ensure_extend_call_id(act_kind: Bytecode, id: u8) -> VmrtErr {
    // Runtime allowlist: bytecode can be crafted directly, so we must reject unknown ids here
    // (compile-time checks in the language layer are not sufficient).
    let ok = match act_kind {
        Bytecode::EXTACTION => search_ext_by_id(id, &CALL_EXTEND_ACTION_DEFS).is_some(),
        Bytecode::EXTENV    => search_ext_by_id(id, &CALL_EXTEND_ENV_DEFS).is_some(),
        Bytecode::EXTVIEW   => search_ext_by_id(id, &CALL_EXTEND_VIEW_DEFS).is_some(),
        _ => false,
    };
    if !ok {
        return Err(ItrErr::new(
            ItrErrCode::ExtActCallError,
            &format!("extend id {} not found", id),
        ));
    }
    Ok(())
}

pub fn ensure_extend_call_allowed(mode: ExecMode, act_kind: Bytecode, id: u8) -> VmrtErr {
    ensure_extend_call_id(act_kind, id)?;
    if act_kind == Bytecode::EXTACTION && mode != ExecMode::Main {
        return Err(ItrErr::new(
            ItrErrCode::ExtActDisabled,
            "extend action not support in non-main call",
        ));
    }
    if mode == ExecMode::Pure {
        match act_kind {
            Bytecode::EXTENV => {
                return Err(ItrErr::new(
                    ItrErrCode::ExtActDisabled,
                    "extend env call not support in pure call",
                ));
            }
            Bytecode::EXTVIEW => {
                return Err(ItrErr::new(
                    ItrErrCode::ExtActDisabled,
                    "extend view call not support in pure call",
                ));
            }
            _ => {}
        }
    }
    Ok(())
}

#[cfg(test)]
mod extact_tests {
    use super::*;

    #[test]
    fn extaction_disallowed_in_view_mode() {
        let action_id = CALL_EXTEND_ACTION_DEFS[0].0;
        let err = ensure_extend_call_allowed(ExecMode::View, Bytecode::EXTACTION, action_id)
            .unwrap_err();
        assert_eq!(err.0, ItrErrCode::ExtActDisabled);
    }

    #[test]
    fn extaction_allowed_in_main_mode() {
        let action_id = CALL_EXTEND_ACTION_DEFS[0].0;
        assert!(ensure_extend_call_allowed(ExecMode::Main, Bytecode::EXTACTION, action_id).is_ok());
    }
}
