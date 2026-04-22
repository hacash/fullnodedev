use mint::action::DiaInscEdit;
use protocol::action::*;
use super::action::*;

type ActionDefTy = (u8, &'static str, ValueTy, usize);

const ACTION_UNKNOWN_NAME: &str = "__unknown__";

fn action_kind_name(act_kind: Bytecode) -> &'static str {
    match act_kind {
        Bytecode::ACTION => "ACTION",
        Bytecode::ACTENV => "ACTENV",
        Bytecode::ACTVIEW => "ACTVIEW",
        _ => "ACTION?",
    }
}

pub const ACTION_DEFS: [ActionDefTy; 14] = [
    (HacToTrs::IDX,      "transfer_hac_to",         ValueTy::Nil, 2),
    (HacFromTrs::IDX,    "transfer_hac_from",       ValueTy::Nil, 2),
    (HacFromToTrs::IDX,  "transfer_hac_from_to",    ValueTy::Nil, 3),
    (SatToTrs::IDX,      "transfer_sat_to",         ValueTy::Nil, 2),
    (SatFromTrs::IDX,    "transfer_sat_from",       ValueTy::Nil, 2),
    (SatFromToTrs::IDX,  "transfer_sat_from_to",    ValueTy::Nil, 3),
    (DiaSingleTrs::IDX,  "transfer_hacd_single_to", ValueTy::Nil, 2),
    (DiaToTrs::IDX,      "transfer_hacd_to",        ValueTy::Nil, 2),
    (DiaFromTrs::IDX,    "transfer_hacd_from",      ValueTy::Nil, 2),
    (DiaFromToTrs::IDX,  "transfer_hacd_from_to",   ValueTy::Nil, 3),
    (DiaInscEdit::IDX,   "hacd_insc_edit",          ValueTy::Nil, 5),
    (AssetToTrs::IDX,    "transfer_asset_to",       ValueTy::Nil, 2),
    (AssetFromTrs::IDX,  "transfer_asset_from",     ValueTy::Nil, 2),
    (AssetFromToTrs::IDX,"transfer_asset_from_to",  ValueTy::Nil, 3),
];

pub const ACTION_ENV_DEFS: [ActionDefTy; 3] = [
    (EnvHeight::IDX,          "block_height",          ValueTy::U64,     0),
    (EnvBlockAuthorAddr::IDX, "block_author_addr",     ValueTy::Address, 0),
    (EnvMainAddr::IDX,        "tx_main_addr",          ValueTy::Address, 0),
];

pub const ACTION_VIEW_DEFS: [ActionDefTy; 7] = [
    (ViewBalance::IDX,         "balance",           ValueTy::Bytes, 1),
    (ViewAssetBalance::IDX,    "asset_balance",     ValueTy::U64,   2),
    (ViewCheckSign::IDX,       "check_signature",   ValueTy::Bool,  1),
    (ViewDiaInscNum::IDX,      "hacd_insc_num",     ValueTy::U8,    1),
    (ViewDiaInscGet::IDX,      "hacd_insc_get",     ValueTy::Bytes, 2),
    (ViewDiaNameList::IDX,     "hacd_name_list",    ValueTy::Bytes, 3),
    (ViewDiaOwnerAddrs::IDX,   "hacd_owner_addrs",  ValueTy::Bytes, 1),
];

pub fn search_act_by_id<'a>(id: u8, exts: &'a[ActionDefTy]) -> Option<&'a ActionDefTy> {
    for a in exts {
        if a.0 == id {
            return Some(a)
        }
    }
    None
}

pub fn search_act_name_by_id(id: u8, exts: &[ActionDefTy]) -> &'static str {
    match search_act_by_id(id, exts) {
        Some(a) => a.1,
        _ => ACTION_UNKNOWN_NAME,
    }
}

pub fn ensure_act_id(act_kind: Bytecode, id: u8) -> VmrtErr {
    let ok = match act_kind {
        Bytecode::ACTION => search_act_by_id(id, &ACTION_DEFS).is_some(),
        Bytecode::ACTENV => search_act_by_id(id, &ACTION_ENV_DEFS).is_some(),
        Bytecode::ACTVIEW => search_act_by_id(id, &ACTION_VIEW_DEFS).is_some(),
        _ => false,
    };
    if !ok {
        return Err(ItrErr::new(
            ItrErrCode::InstParamsErr,
            &format!("{} id {} not found", action_kind_name(act_kind), id),
        ));
    }
    Ok(())
}

pub fn ensure_act_allowed(exec: ExecCtx, act_kind: Bytecode, id: u8) -> VmrtErr {
    ensure_act_id(act_kind, id)?;
    if act_kind == Bytecode::ACTION {
        if exec.entry != EntryKind::Main || exec.effect != EffectMode::Edit || !exec.is_outer_entry() {
            return Err(ItrErr::new(
                ItrErrCode::ActDisabled,
                "action not supported in current call context",
            ));
        }
    }
    if exec.effect == EffectMode::Pure {
        match act_kind {
            Bytecode::ACTENV => {
                return Err(ItrErr::new(
                    ItrErrCode::ActDisabled,
                    "action env call not supported in pure call",
                ));
            }
            Bytecode::ACTVIEW => {
                return Err(ItrErr::new(
                    ItrErrCode::ActDisabled,
                    "action view call not supported in pure call",
                ));
            }
            _ => {}
        }
    }
    Ok(())
}


pub const fn act_pass_body(act_kind: Bytecode) -> bool {
    matches!(act_kind, Bytecode::ACTION | Bytecode::ACTVIEW)
}

pub const fn act_have_retv(act_kind: Bytecode) -> bool {
    !matches!(act_kind, Bytecode::ACTION)
}

pub fn act_retv_type(act_kind: Bytecode, idx: u8) -> VmrtRes<ValueTy> {
    let def = match act_kind {
        Bytecode::ACTENV => search_act_by_id(idx, &ACTION_ENV_DEFS),
        Bytecode::ACTVIEW => search_act_by_id(idx, &ACTION_VIEW_DEFS),
        _ => None,
    }
    .ok_or_else(|| ItrErr::new(ItrErrCode::InstParamsErr, &format!("{} id {} not found", action_kind_name(act_kind), idx)))?;
    Ok(def.2)
}










#[cfg(test)]
mod action_call_tests {
    use super::*;

    #[test]
    fn action_disallowed_in_view_mode() {
        let action_id = ACTION_DEFS[0].0;
        let err = ensure_act_allowed(ExecCtx::view(), Bytecode::ACTION, action_id)
            .unwrap_err();
        assert_eq!(err.0, ItrErrCode::ActDisabled);
    }

    #[test]
    fn action_allowed_in_main_mode() {
        let action_id = ACTION_DEFS[0].0;
        assert!(ensure_act_allowed(ExecCtx::main(), Bytecode::ACTION, action_id).is_ok());
    }

    #[test]
    fn actenv_author_is_registered_and_allowed() {
        let env_id = EnvBlockAuthorAddr::IDX;
        let def = search_act_by_id(env_id, &ACTION_ENV_DEFS)
            .expect("EnvBlockAuthorAddr must exist in ACTION_ENV_DEFS");
        assert_eq!(def.1, "block_author_addr");
        assert!(ensure_act_allowed(ExecCtx::main(), Bytecode::ACTENV, env_id).is_ok());
    }

    #[test]
    fn hacd_insc_edit_action_def_is_registered() {
        let def = search_act_by_id(DiaInscEdit::IDX, &ACTION_DEFS)
            .expect("DiaInscEdit must exist in ACTION_DEFS");
        assert_eq!(def.1, "hacd_insc_edit");
        assert_eq!(def.3, 4);
    }

    #[test]
    fn unknown_action_ids_are_instruction_param_errors() {
        let err = ensure_act_id(Bytecode::ACTION, u8::MAX).unwrap_err();
        assert_eq!(err.0, ItrErrCode::InstParamsErr);
        assert!(err.1.contains("ACTION id 255 not found"));

        let err = act_retv_type(Bytecode::ACTVIEW, u8::MAX).unwrap_err();
        assert_eq!(err.0, ItrErrCode::InstParamsErr);
        assert!(err.1.contains("ACTVIEW id 255 not found"));
    }
}
