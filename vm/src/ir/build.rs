pub fn convert_ir_to_bytecode(bytes: &[u8]) -> VmrtRes<Vec<u8>> {
    // Parse as raw block content (without IRBLOCK header) Input format: [node1][node2]... (no opcode/length prefix)
    let block = parse_ir_block(bytes, &mut 0)?;
    block.codegen()
}

pub fn verify_ir_runtime_safe_bytecodes(codes: &[u8]) -> VmrtErr {
    let mut i = 0usize;
    let mut last = None;
    while i < codes.len() {
        let inst: Bytecode = std_mem_transmute!(codes[i]);
        last = Some(inst);
        let meta = inst.metadata();
        if !meta.valid {
            return itr_err_fmt!(InstInvalid, "bytecode {} not found", inst as u8);
        }
        if matches!(inst, JMPL | BRL) {
            return itr_err_fmt!(
                InstInvalid,
                "absolute jumps are not allowed in IRNode code; use relative jumps"
            );
        }
        i += 1;
        let end = match inst {
            PBUF => {
                if i >= codes.len() {
                    return itr_err_code!(InstParamsErr);
                }
                i + 1 + codes[i] as usize
            }
            PBUFL => {
                if i + 2 > codes.len() {
                    return itr_err_code!(InstParamsErr);
                }
                let len = u16::from_be_bytes(codes[i..i + 2].try_into().unwrap()) as usize;
                i + 2 + len
            }
            _ => i + meta.param as usize,
        };
        if end > codes.len() {
            return itr_err_code!(InstParamsErr);
        }
        i = end;
    }
    let Some(last) = last else {
        return itr_err_code!(CodeEmpty);
    };
    ensure_terminal_instruction(last)?;
    Ok(())
}

pub fn convert_ir_to_runtime_bytecode(bytes: &[u8]) -> VmrtRes<Vec<u8>> {
    let codes = convert_ir_to_bytecode(bytes)?;
    verify_ir_runtime_safe_bytecodes(&codes)?;
    Ok(codes)
}

pub fn runtime_irs_to_bytecodes(bytes: &[u8], gas_extra: &GasExtra) -> VmrtRes<Vec<u8>> {
    runtime_irs_to_exec_bytecodes(bytes, gas_extra)
}

pub fn runtime_irs_to_exec_bytecodes(bytes: &[u8], _gas_extra: &GasExtra) -> VmrtRes<Vec<u8>> {
    let codes = convert_ir_to_runtime_bytecode(bytes)?;
    // Runtime executable stream is the compiled code only. IR-format gas is charged
    // at frame entry from raw IR length, so cached bytecode stays independent from gas policy.
    Ok(codes)
}

#[cfg(test)]
mod ir_runtime_codegen_tests {
    use super::*;

    #[test]
    fn irnode_runtime_rejects_absolute_jumps_before_burn_prefix_can_shift_them() {
        let raw = vec![
            IRBYTECODE as u8,
            0,
            8,
            P1 as u8,
            JMPL as u8,
            0,
            7,
            P2 as u8,
            RET as u8,
            P3 as u8,
            RET as u8,
        ];
        let plain = convert_ir_to_bytecode(&raw).expect("plain IR conversion should still work");
        assert_eq!(
            plain,
            vec![P1 as u8, JMPL as u8, 0, 7, P2 as u8, RET as u8, P3 as u8, RET as u8,]
        );

        let err = convert_ir_to_runtime_bytecode(&raw).unwrap_err();
        assert_eq!(err.0, ItrErrCode::InstInvalid);
        assert!(err.1.contains("absolute jumps are not allowed"));
    }

    #[test]
    fn irnode_runtime_allows_relative_jumps_used_by_codegen() {
        let raw = vec![
            IRBYTECODE as u8,
            0,
            6,
            P1 as u8,
            JMPSL as u8,
            0,
            1,
            P2 as u8,
            RET as u8,
        ];
        convert_ir_to_runtime_bytecode(&raw).expect("relative jumps stay valid in compiled IR");
    }

    #[test]
    fn irnode_runtime_requires_converted_code_to_be_terminal() {
        let raw = vec![IRBYTECODE as u8, 0, 1, P1 as u8];
        let err = convert_ir_to_runtime_bytecode(&raw).unwrap_err();
        assert_eq!(err.0, ItrErrCode::CodeNotWithEnd);
    }

    #[test]
    fn irnode_exec_bytecode_does_not_append_end_after_tail_call() {
        let selector = [1, 2, 3, 4];
        let mut raw_call = vec![PNIL as u8, CALLSELF as u8];
        raw_call.extend_from_slice(&selector);

        let mut raw = vec![IRBYTECODE as u8];
        raw.extend_from_slice(&(raw_call.len() as u16).to_be_bytes());
        raw.extend_from_slice(&raw_call);

        let exec = runtime_irs_to_exec_bytecodes(&raw, &GasExtra::new(1)).unwrap();
        assert_eq!(exec, raw_call);
        assert_ne!(exec.last().copied(), Some(END as u8));
    }

    #[test]
    fn irnode_exec_bytecode_has_no_burn_prefix() {
        let raw = vec![IRBYTECODE as u8, 0, 1, END as u8];
        let exec = runtime_irs_to_exec_bytecodes(&raw, &GasExtra::new(1)).unwrap();
        assert_eq!(exec, vec![END as u8]);
    }
}
