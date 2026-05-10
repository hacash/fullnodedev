pub fn convert_ir_to_bytecode(bytes: &[u8]) -> VmrtRes<Vec<u8>> {
    // Parse as raw block content (without IRBLOCK header) Input format: [node1][node2]... (no opcode/length prefix)
    let block = parse_ir_block(bytes, &mut 0)?;
    block.codegen()
}

pub fn verify_ir_runtime_safe_bytecodes(codes: &[u8]) -> VmrtErr {
    let mut i = 0usize;
    while i < codes.len() {
        let inst: Bytecode = std_mem_transmute!(codes[i]);
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

pub fn runtime_irs_to_exec_bytecodes(bytes: &[u8], gas_extra: &GasExtra) -> VmrtRes<Vec<u8>> {
    let codes = convert_ir_to_runtime_bytecode(bytes)?;
    // Runtime executable stream is BURN(compile_fee) + code + END, where compile_fee uses chain gas config formula.
    let exec_len = codes.len() + 1; // include END
    let fee = gas_extra.compile_bytes(exec_len);
    if fee < 0 || fee > u16::MAX as i64 {
        return itr_err_fmt!(GasError, "IR compile fee overflow: {}", fee);
    }
    let mut rescodes = Vec::with_capacity(exec_len + 3);
    rescodes.push(BURN as u8);
    rescodes.extend_from_slice(&(fee as u16).to_be_bytes());
    rescodes.extend_from_slice(&codes);
    rescodes.push(END as u8);
    Ok(rescodes)
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
        convert_ir_to_runtime_bytecode(&raw).expect("relative jumps stay valid after BURN prefix");
    }
}
