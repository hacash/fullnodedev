

pub fn convert_ir_to_bytecode(bytes: &[u8]) -> VmrtRes<Vec<u8>> {
    // Parse as raw block content (without IRBLOCK header)
    // Input format: [node1][node2]... (no opcode/length prefix)
    let block = parse_ir_block(bytes, &mut 0)?;
    block.codegen()
}

pub fn runtime_irs_to_bytecodes(bytes: &[u8]) -> VmrtRes<Vec<u8>> {
    let codes = convert_ir_to_bytecode(bytes)?;
    let mut rescodes = Vec::with_capacity(codes.len() + 4);
    // append burn gas & end
    let cdl = ((codes.len() / 8) as u16).to_be_bytes(); // burn gas = size / 8
    rescodes.push(BURN as u8);
    rescodes.extend_from_slice(&cdl);
    rescodes.extend_from_slice(&codes); // code body
    rescodes.push(END as u8);
    Ok(rescodes)
}
