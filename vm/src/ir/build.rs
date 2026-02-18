

pub fn convert_ir_to_bytecode(bytes: &[u8]) -> VmrtRes<Vec<u8>> {
    // Parse as raw block content (without IRBLOCK header) Input format: [node1][node2]... (no opcode/length prefix)
    let block = parse_ir_block(bytes, &mut 0)?;
    block.codegen()
}

pub fn runtime_irs_to_bytecodes(bytes: &[u8], height: u64) -> VmrtRes<Vec<u8>> {
    runtime_irs_to_exec_bytecodes(bytes, height)
}

pub fn runtime_irs_to_exec_bytecodes(bytes: &[u8], height: u64) -> VmrtRes<Vec<u8>> {
    let codes = convert_ir_to_bytecode(bytes)?;
    // Runtime executable stream is BURN(compile_fee) + code + END, where compile_fee uses chain gas config formula.
    let exec_len = codes.len() + 1; // include END
    let fee = GasExtra::new(height).compile_bytes(exec_len);
    if fee < 0 || fee > u16::MAX as i64 {
        return itr_err_fmt!(GasError, "IR compile fee overflow: {}", fee)
    }
    let mut rescodes = Vec::with_capacity(exec_len + 3);
    rescodes.push(BURN as u8);
    rescodes.extend_from_slice(&(fee as u16).to_be_bytes());
    rescodes.extend_from_slice(&codes);
    rescodes.push(END as u8);
    Ok(rescodes)
}
