
pub fn push_empty() -> Box<dyn IRNode> {
    Box::new(IRNodeEmpty {})
}

pub fn push_nil() -> Box<dyn IRNode> {
    use Bytecode::*;
    push_inst(PNIL)
}

pub fn push_local_get(i: u8, text: String) -> Box<dyn IRNode> {
    use Bytecode::*;
    match i {
        0 => Box::new(IRNodeLeaf { hrtv: true, inst: GET0, text }),
        1 => Box::new(IRNodeLeaf { hrtv: true, inst: GET1, text }),
        2 => Box::new(IRNodeLeaf { hrtv: true, inst: GET2, text }),
        3 => Box::new(IRNodeLeaf { hrtv: true, inst: GET3, text }),
        _ => Box::new(IRNodeParam1 { hrtv: true, inst: GET, text, para: i }),
    }
}

pub fn push_inst_noret(inst: Bytecode) -> Box<dyn IRNode> {
    Box::new(IRNodeLeaf::notext(false, inst))
}

pub fn push_inst(inst: Bytecode) -> Box<dyn IRNode> {
    Box::new(IRNodeLeaf::notext(true, inst))
}

pub fn push_single(inst: Bytecode, subx: Box<dyn IRNode>) -> Box<dyn IRNode> {
    Box::new(IRNodeSingle{inst, hrtv: true, subx})
}

pub fn push_single_noret(inst: Bytecode, subx: Box<dyn IRNode>) -> Box<dyn IRNode> {
    Box::new(IRNodeSingle{inst, hrtv: false, subx})
}

pub fn push_num(n: u128) -> Box<dyn IRNode> {
    use Bytecode::*;
    macro_rules! push_uint {
        ($n:expr, $t:expr) => {{
            let buf = buf_drop_left_zero(&$n.to_be_bytes(), 0);
            let numv = iter::once(buf.len() as u8).chain(buf).collect::<Vec<_>>();
            Box::new(IRNodeSingle {
                hrtv: true,
                inst: $t,
                subx: Box::new(IRNodeParams {
                    hrtv: true,
                    inst: PBUF,
                    para: numv,
                }),
            })
        }};
    }

    match n {
        0 => push_inst(P0),
        1 => push_inst(P1),
        2 => push_inst(P2),
        3 => push_inst(P3),
        4..=255 => Box::new(IRNodeParam1 {
            hrtv: true,
            inst: PU8,
            para: n as u8,
            text: String::new(),
        }),
        256..=65535 => Box::new(IRNodeParam2 {
            hrtv: true,
            inst: PU16,
            para: (n as u16).to_be_bytes(),
        }),
        65536..=4294967295 => push_uint!(n, CU32),
        4294967296..=18446744073709551615 => push_uint!(n, CU64),
        _ => push_uint!(n, CU128),
    }
}

pub fn push_addr(a: field::Address) -> Box<dyn IRNode> {
    use Bytecode::*;
    let para = vec![vec![field::Address::SIZE as u8], a.serialize()].concat();
    Box::new(IRNodeParam1Single {
        hrtv: true,
        inst: CTO,
        para: ValueTy::Address as u8,
        subx: Box::new(IRNodeParams {
            hrtv: true,
            inst: PBUF,
            para,
        }),
    })
}

pub fn push_bytes(b: &Vec<u8>) -> Ret<Box<dyn IRNode>> {
    use Bytecode::*;
    let bl = b.len();
    if bl == 0 {
        return Ok(push_inst(PNBUF));
    }
    if bl > u16::MAX as usize {
        return errf!("bytes data too long");
    }
    let isl = bl > u8::MAX as usize;
    let inst = maybe!(isl, PBUFL, PBUF);
    let size = maybe!(isl, (bl as u16).to_be_bytes().to_vec(), vec![bl as u8]);
    let para = std::iter::empty().chain(size).chain(b.clone()).collect::<Vec<_>>();
    Ok(Box::new(IRNodeParams {
        hrtv: true,
        inst,
        para,
    }))
}

/// Drop the outer IR block wrapper from `IRNodeArray::serialize()` output.
///
/// The serialized form for a top-level block is:
/// `[IRBLOCK|IRBLOCKR][child_count:u16][children...]`.
///
/// For stored ircode we keep only `children...` (no outer wrapper), so that
/// `parse_ir_block()` can parse nodes until EOF.
pub fn drop_irblock_wrap(mut serialized: Vec<u8>) -> Ret<Vec<u8>> {
    if serialized.len() < 3 {
        return errf!("invalid serialized IR: length {} < 3", serialized.len());
    }
    let op = serialized[0];
    if op != Bytecode::IRBLOCK as u8 && op != Bytecode::IRBLOCKR as u8 {
        return errf!(
            "invalid serialized IR: expected IRBLOCK/IRBLOCKR header, got opcode {}",
            op
        );
    }
    Ok(serialized.split_off(3))
}
