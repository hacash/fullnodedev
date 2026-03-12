use crate::native::{NativeEnv, NativeFunc};
use crate::value::{parse_cto_target_ty_param, parse_value_ty_param};

/*
    Verify bytecode validity and return the instruction table.
*/
pub fn convert_and_check(
    cap: &SpaceCap,
    ctype: CodeType,
    codes: &[u8],
    height: u64,
) -> VmrtRes<Vec<u8>> {
    use CodeType::*;
    let bytecodes = match ctype {
        IRNode => &runtime_irs_to_bytecodes(codes, height)?,
        Bytecode => codes,
    };
    // check size
    if bytecodes.len() > cap.function_size {
        return itr_err_code!(CodeTooLong);
    }
    // verify inst
    verify_bytecodes_with_limits(bytecodes, cap.value_size)
}

pub fn verify_bytecodes(codes: &[u8]) -> VmrtRes<Vec<u8>> {
    verify_bytecodes_with_limits(codes, SpaceCap::new(1).value_size)
}

fn verify_bytecodes_with_limits(codes: &[u8], max_push_buf_len: usize) -> VmrtRes<Vec<u8>> {
    // check empty
    let cl = codes.len();
    if cl <= 0 {
        return itr_err_code!(CodeEmpty);
    }
    if cl > u16::MAX as usize {
        return itr_err_code!(CodeTooLong);
    }
    // check inst valid
    let (instable, jumpdests) = verify_valid_instruction(codes, max_push_buf_len)?;
    // check jump dests
    verify_jump_dests(&instable, &jumpdests)?;
    // ok finish
    Ok(instable)
}

/// Ensure the last instruction is a terminal one (RET/END/ERR/ABT or exposed call opcode).
/// Failure (CodeNotWithEnd) is a fitsh code compile error and propagates to the compiler
/// via compile_body -> parse_function -> parse_top_level -> fitshc::compile.
fn ensure_terminal_instruction(inst: Bytecode) -> VmrtErr {
    if matches!(inst, RET | END | ERR | ABT) || is_user_call_inst(inst) {
        return Ok(());
    };
    // error
    itr_err_code!(CodeNotWithEnd)
}

/*

*/
fn verify_valid_instruction(
    codes: &[u8],
    max_push_buf_len: usize,
) -> VmrtRes<(Vec<u8>, Vec<isize>)> {
    // use Bytecode::*;
    let cdlen = codes.len(); // end/ret/err/abt in tail
    let mut instable = vec![0u8; cdlen];
    let mut jumpdest = vec![];
    let mut i = 0;
    let mut cur = Bytecode::default();
    while i < cdlen {
        let curbt = codes[i];
        let inst: Bytecode = std_mem_transmute!(curbt);
        let meta = inst.metadata();
        if !meta.valid {
            return itr_err_fmt!(InstInvalid, "{}", inst as u8);
        }
        match inst {
            IRBYTECODE | IRLIST | IRBLOCK | IRBLOCKR | IRIF | IRIFR | IRWHILE | IRBREAK
            | IRCONTINUE => {
                return itr_err_fmt!(InstInvalid, "IR bytecode {:?} is not allowed", inst)
            }
            _ => {}
        }
        cur = inst;
        instable[i] = 1; // yes is valid instruction
        i += 1;
        macro_rules! pu8 {
            () => {{
                if i >= cdlen {
                    return itr_err_code!(InstParamsErr);
                }
                codes[i as usize]
            }};
        }
        macro_rules! pu16 {
            () => {{
                let r = i + 2;
                if r > cdlen {
                    return itr_err_code!(InstParamsErr);
                }
                u16::from_be_bytes(codes[i as usize..r as usize].try_into().unwrap())
            }};
        }
        macro_rules! pi8 {
            () => {
                pu8!() as i8
            };
        }
        macro_rules! pi16 {
            () => {
                pu16!() as i16
            };
        }
        macro_rules! adddest {
            ($jt:expr) => {{
                jumpdest.push($jt)
            }};
        }
        match inst {
            // push buf
            PBUF => {
                let l = pu8!() as usize;
                if l > max_push_buf_len {
                    return itr_err_fmt!(InstParamsErr, "PBUF size {} too large", l);
                }
                i += l
            }
            PBUFL => {
                let l = pu16!() as usize;
                if l > max_push_buf_len {
                    return itr_err_fmt!(InstParamsErr, "PBUFL size {} too large", l);
                }
                i += l
            }
            // ext/native
            ACTION | ACTENV | ACTVIEW => ensure_act_id(inst, pu8!())?,
            NTFUNC => {
                let idx = pu8!();
                if !NativeFunc::has_idx(idx) {
                    return itr_err_fmt!(NativeFuncError, "native func idx {} not found", idx);
                }
            }
            NTENV => {
                let idx = pu8!();
                if !NativeEnv::has_idx(idx) {
                    return itr_err_fmt!(NativeEnvError, "native env idx {} not found", idx);
                }
            }
            CTO => {
                let _ = parse_cto_target_ty_param(pu8!())?;
            }
            TIS => {
                let _ = parse_value_ty_param(pu8!())?;
            }
            _ if is_user_call_inst(inst) => {
                let r = i + meta.param as usize;
                if r > cdlen {
                    return itr_err_code!(InstParamsErr);
                }
                let _ = decode_user_call_site(inst, &codes[i..r])?;
            }
            // jump record
            JMPL | BRL => adddest!(pu16!() as isize),
            JMPS | BRS => adddest!(i as isize + pi8!() as isize + 1),
            JMPSL | BRSL | BRSLN => adddest!(i as isize + pi16!() as isize + 2),
            _ => {}
        };
        i += meta.param as usize;
        if i > cdlen {
            return itr_err_code!(InstParamsErr);
        }
        // next
    }
    ensure_terminal_instruction(cur)?; // check end
                                       // finish orr
    Ok((instable, jumpdest))
}

//
fn verify_jump_dests(instable: &[u8], jumpdests: &[isize]) -> VmrtErr {
    let itlen = instable.len();
    let right = itlen as isize - 1;
    for jp in jumpdests {
        let j = *jp;
        if j < 0 || j > right {
            return itr_err_code!(JumpOverflow);
        }
        if 0 == instable[j as usize] {
            return itr_err_code!(JumpInDataSeg);
        }
    }
    // finish
    Ok(())
}

#[cfg(test)]
mod verify_type_param_tests {
    use super::*;

    #[test]
    fn verify_rejects_unknown_type_id_for_tis_and_cto() {
        let unknown_ids = [12u8];
        for raw in unknown_ids {
            let tis_codes = vec![
                Bytecode::P0 as u8,
                Bytecode::TIS as u8,
                raw,
                Bytecode::END as u8,
            ];
            let cto_codes = vec![
                Bytecode::P0 as u8,
                Bytecode::CTO as u8,
                raw,
                Bytecode::END as u8,
            ];
            let r1 = verify_bytecodes(&tis_codes);
            let r2 = verify_bytecodes(&cto_codes);
            assert!(matches!(r1, Err(ItrErr(ItrErrCode::InstParamsErr, _))));
            assert!(matches!(r2, Err(ItrErr(ItrErrCode::InstParamsErr, _))));
        }
    }

    #[test]
    fn verify_rejects_reserved_type_id_for_tis_and_cto() {
        let tis_codes = vec![
            Bytecode::P0 as u8,
            Bytecode::TIS as u8,
            RESERVED_U256_TYPE_ID,
            Bytecode::END as u8,
        ];
        let cto_codes = vec![
            Bytecode::P0 as u8,
            Bytecode::CTO as u8,
            RESERVED_U256_TYPE_ID,
            Bytecode::END as u8,
        ];
        let r1 = verify_bytecodes(&tis_codes);
        let r2 = verify_bytecodes(&cto_codes);
        assert!(matches!(r1, Err(ItrErr(ItrErrCode::InstParamsErr, _))));
        assert!(matches!(r2, Err(ItrErr(ItrErrCode::InstParamsErr, _))));
    }

    #[test]
    fn verify_accepts_active_type_id_for_tis_and_cto() {
        let tis_codes = vec![
            Bytecode::P0 as u8,
            Bytecode::TIS as u8,
            ValueTy::U8 as u8,
            Bytecode::END as u8,
        ];
        let cto_codes = vec![
            Bytecode::P0 as u8,
            Bytecode::CTO as u8,
            ValueTy::U8 as u8,
            Bytecode::END as u8,
        ];
        assert!(verify_bytecodes(&tis_codes).is_ok());
        assert!(verify_bytecodes(&cto_codes).is_ok());
    }

    #[test]
    fn verify_accepts_declared_type_ids_for_tis() {
        let types = [
            ValueTy::Nil,
            ValueTy::Bool,
            ValueTy::U8,
            ValueTy::U16,
            ValueTy::U32,
            ValueTy::U64,
            ValueTy::U128,
            ValueTy::Bytes,
            ValueTy::Address,
            ValueTy::HeapSlice,
            ValueTy::Args,
            ValueTy::Compo,
        ];
        for ty in types {
            let tis_codes = vec![
                Bytecode::P0 as u8,
                Bytecode::TIS as u8,
                ty as u8,
                Bytecode::END as u8,
            ];
            assert!(
                verify_bytecodes(&tis_codes).is_ok(),
                "TIS should accept declared type id {:?}",
                ty
            );
        }
    }

    #[test]
    fn verify_accepts_cto_targets_in_cast_set() {
        let cast_set = [
            ValueTy::Bool,
            ValueTy::U8,
            ValueTy::U16,
            ValueTy::U32,
            ValueTy::U64,
            ValueTy::U128,
            ValueTy::Bytes,
            ValueTy::Address,
        ];
        for ty in cast_set {
            let cto_codes = vec![
                Bytecode::P0 as u8,
                Bytecode::CTO as u8,
                ty as u8,
                Bytecode::END as u8,
            ];
            assert!(
                verify_bytecodes(&cto_codes).is_ok(),
                "CTO should accept cast target {:?}",
                ty
            );
        }
    }

    #[test]
    fn verify_rejects_cto_targets_outside_cast_set() {
        for ty in [
            ValueTy::Nil,
            ValueTy::HeapSlice,
            ValueTy::Args,
            ValueTy::Compo,
        ] {
            let cto_codes = vec![
                Bytecode::P0 as u8,
                Bytecode::CTO as u8,
                ty as u8,
                Bytecode::END as u8,
            ];
            let res = verify_bytecodes(&cto_codes);
            assert!(matches!(res, Err(ItrErr(ItrErrCode::InstParamsErr, _))));
        }
    }
}

#[cfg(test)]
mod call_verify_tests {
    use super::*;

    #[test]
    fn verify_accepts_generic_call_slot() {
        let body = encode_call_body(
            CallTarget::Upper,
            EffectMode::Edit,
            [0x01, 0x02, 0x03, 0x04],
        );
        let mut codes = vec![Bytecode::CALL as u8];
        codes.extend_from_slice(&body);
        codes.push(Bytecode::END as u8);
        verify_bytecodes(&codes).unwrap();
    }

    #[test]
    fn verify_accepts_codecall_without_linear_end_guard() {
        let body = encode_splice_body(1, [0x01, 0x02, 0x03, 0x04]);
        let mut codes = vec![Bytecode::P0 as u8, Bytecode::CODECALL as u8];
        codes.extend_from_slice(&body);
        codes.push(Bytecode::END as u8);
        verify_bytecodes(&codes).unwrap();
    }
}
