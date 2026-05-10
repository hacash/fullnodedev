/* parse ir list */
pub fn parse_ir_list(stuff: &[u8], seek: &mut usize) -> VmrtRes<IRNodeArray> {
    let u16max = u16::MAX as usize;
    let codelen = stuff.len();
    if codelen > u16max {
        return itr_err_code!(CodeTooLong);
    }
    let mut list = IRNodeArray::new_list();
    loop {
        let pres = parse_ir_node(stuff, seek)?;
        let Some(irnode) = pres else {
            break; // end
        };
        list.push(irnode);
    }
    // finish
    Ok(list)
}

/* parse ir block */
pub fn parse_ir_block(stuff: &[u8], seek: &mut usize) -> VmrtRes<IRNodeArray> {
    let u16max = u16::MAX as usize;
    let codelen = stuff.len();
    if codelen > u16max {
        return itr_err_code!(CodeTooLong);
    }
    let mut block = IRNodeArray::new_block();
    loop {
        let pres = parse_ir_node(stuff, seek)?;
        let Some(irnode) = pres else {
            break; // end
        };
        block.push(irnode);
    }
    // finish
    Ok(block)
}

/* * * parse one node (public interface for serialized IR) */
pub fn parse_ir_node_one(stuff: &[u8], seek: &mut usize) -> VmrtRes<Box<dyn IRNode>> {
    parse_ir_node_must(stuff, seek, 0, false)
}

/* * * parse one node */
fn parse_ir_node(stuff: &[u8], seek: &mut usize) -> VmrtRes<Option<Box<dyn IRNode>>> {
    let codesz = stuff.len();
    if codesz == 0 || *seek >= codesz {
        return Ok(None); // finish end
    }
    Ok(Some(parse_ir_node_must(stuff, seek, 0, false)?))
}

// must
fn parse_ir_node_must(
    stuff: &[u8],
    seek: &mut usize,
    depth: usize,
    isrtv: bool,
) -> VmrtRes<Box<dyn IRNode>> {
    if depth > 32 {
        return itr_err_code!(IRNodeOverDepth);
    }

    let codesz = stuff.len();
    if codesz == 0 || *seek >= codesz {
        return itr_err_code!(CodeOverflow);
    }

    // code
    let insbyte = stuff[*seek]; // u8
    let inst: Bytecode = std_mem_transmute!(insbyte);
    // parse
    let irnode: Box<dyn IRNode>;
    // mv sk
    *seek += 1;

    macro_rules! itrp1 {
        () => {{
            let _r = *seek + 1;
            if _r > codesz {
                return itr_err_code!(CodeOverflow);
            }
            let bt = stuff[*seek];
            *seek = _r;
            bt
        }};
    }

    macro_rules! itrp2 {
        () => {{
            let _r = *seek + 2;
            if _r > codesz {
                return itr_err_code!(CodeOverflow);
            }
            let bts: [u8; 2] = stuff[*seek.._r].try_into().unwrap();
            *seek = _r;
            bts
        }};
    }

    macro_rules! itrbuf {
        ($l: expr) => {{
            let _r = *seek + $l;
            if _r > codesz {
                return itr_err_code!(CodeOverflow);
            }
            let bts = stuff[*seek.._r].to_vec();
            *seek = _r;
            bts
        }};
    }

    macro_rules! subdph {
        ($ndp:expr, $rtv:expr) => {
            parse_ir_node_must(stuff, seek, $ndp, $rtv)?
        };
    }

    macro_rules! submust {
        () => {
            subdph!(depth + 1, true)
        };
    }

    macro_rules! param1_multi {
        ($argc:expr, $hrtv:expr, $node:ident { $($field:ident),+ $(,)? }) => {{
            let para = itrp1!();
            validate_param1_multi_arity(inst, para, $hrtv, $argc)?;
            Box::new($node {
                hrtv: $hrtv,
                inst,
                para,
                $($field: submust!()),+
            })
        }}
    }

    // parse
    let meta = inst.metadata();
    if inst == DUPN {
        return itr_err_fmt!(
            InstInvalid,
            "DUPN has dynamic multi-output and is not representable in IRNode"
        );
    }
    let hrtv = meta.output == 1;
    // parse
    irnode = match inst {
        // BYTECODE LIST BLOCK IF WHILE
        IRBYTECODE => {
            let mut bts = IRNodeBytecodes::default();
            let p = itrp2!();
            let n = u16::from_be_bytes(p);
            bts.codes = itrbuf!(n as usize);
            Box::new(bts)
        }
        IRLIST => {
            let mut list = IRNodeArray::with_opcode(inst);
            let p = itrp2!();
            let n = u16::from_be_bytes(p);
            let ndp = depth + 1;
            for _i in 0..n {
                // IRLIST is a sequence container; whether it yields a value depends on its last item.
                list.push(parse_ir_node_must(stuff, seek, ndp, false)?);
            }
            Box::new(list)
        }
        IRBLOCK | IRBLOCKR => {
            let mut block = IRNodeArray::with_opcode(inst);
            let p = itrp2!();
            let n = u16::from_be_bytes(p);
            let ndp = depth + 1;
            if inst == IRBLOCKR && n == 0 {
                return itr_err_fmt!(InstInvalid, "empty block expr");
            }
            for i in 0..n {
                // block statement items do not need to produce values, except: - IRBLOCKR (block expression) requires the last item to produce a value.
                let need_rtv = inst == IRBLOCKR && i + 1 == n;
                block.push(subdph!(ndp, need_rtv));
            }
            Box::new(block)
        }
        IRIF | IRIFR => {
            let ndp = depth + 1;
            let need_branch_rtv = inst == IRIFR;
            Box::new(IRNodeTriple {
                hrtv,
                inst,
                subx: subdph!(ndp, true),
                // if-expression requires both branches to produce values
                suby: subdph!(ndp, need_branch_rtv),
                subz: subdph!(ndp, need_branch_rtv),
            })
        }
        IRWHILE => {
            let ndp = depth + 1;
            Box::new(IRNodeDouble {
                hrtv,
                inst,
                subx: subdph!(ndp, true),
                suby: subdph!(ndp, false),
            })
        }
        PBUF | PBUFL => {
            let (bl, size) = maybe!(
                PBUF == inst,
                {
                    let p1 = itrp1!();
                    (p1 as usize, vec![p1])
                },
                {
                    let p2 = itrp2!();
                    (u16::from_be_bytes(p2) as usize, p2.to_vec())
                }
            );
            let buf = itrbuf!(bl);
            let para: Vec<u8> = iter::empty().chain(size).chain(buf).collect();
            Box::new(IRNodeParams { hrtv, inst, para })
        }
        _ => {
            // other inst
            if !meta.valid {
                return itr_err_fmt!(InstInvalid, "bytecode {} not found", inst as u8);
            }
            if meta.output > 1 && meta.input < 255 {
                return itr_err_fmt!(InstInvalid, "invalid irnode {}", inst as u8);
            }
            match (meta.param, meta.input) {
                // (0, 0) => Box::new(IRNodeLeaf::notext(hrtv, inst)), // leaf
                (0, 1) => Box::new(IRNodeSingle {
                    hrtv,
                    inst,
                    subx: submust!(),
                }),
                (0, 2) => Box::new(IRNodeDouble {
                    hrtv,
                    inst,
                    subx: submust!(),
                    suby: submust!(),
                }),
                (0, 3) => Box::new(IRNodeTriple {
                    hrtv,
                    inst,
                    subx: submust!(),
                    suby: submust!(),
                    subz: submust!(),
                }),
                (0, 4) => Box::new(IRNodeQuad {
                    hrtv,
                    inst,
                    subx: submust!(),
                    suby: submust!(),
                    subz: submust!(),
                    subw: submust!(),
                }),
                (0, 5) => Box::new(IRNodeQuint {
                    hrtv,
                    inst,
                    suba: submust!(),
                    subb: submust!(),
                    subc: submust!(),
                    subd: submust!(),
                    sube: submust!(),
                }),
                (0, 0 | 255) => Box::new(IRNodeLeaf::notext(hrtv, inst)), // leaf
                (1, 2) => param1_multi!(2, hrtv, IRNodeParam1Double { subx, suby }),
                (1, 3) => param1_multi!(3, hrtv, IRNodeParam1Triple { subx, suby, subz }),
                (1, 4) => param1_multi!(
                    4,
                    hrtv,
                    IRNodeParam1Quad {
                        subx,
                        suby,
                        subz,
                        subw
                    }
                ),
                (1, 0 | 255) => Box::new(IRNodeParam1 {
                    hrtv,
                    inst,
                    para: itrp1!(),
                    text: s!(""),
                }), // params one (no IR subtree args)
                (2, 0 | 255) => Box::new(IRNodeParam2 {
                    hrtv,
                    inst,
                    para: itrp2!(),
                }), // params two
                (1, 1) => Box::new(IRNodeParam1Single {
                    hrtv,
                    inst,
                    para: itrp1!(),
                    subx: submust!(),
                }), // params one
                (2, 1) => Box::new(IRNodeParam2Single {
                    hrtv,
                    inst,
                    para: itrp2!(),
                    subx: submust!(),
                }), // params two
                (a, 0) => Box::new(IRNodeParams {
                    hrtv,
                    inst,
                    para: itrbuf!(a as usize),
                }), // params
                (a, 1) => Box::new(IRNodeParamsSingle {
                    hrtv,
                    inst,
                    para: itrbuf!(a as usize),
                    subx: submust!(),
                }),
                _ => {
                    return itr_err_fmt!(
                        InstInvalid,
                        "invalid irnode {:?} of ps={} i={}",
                        inst,
                        meta.param,
                        meta.input
                    )
                }
            }
        }
    };
    // check return value based on the actual parsed node (not just bytecode metadata). This is important for container nodes like IRLIST/IRBLOCK whose return-value-ness is contextual.
    if isrtv && !irnode.hasretval() {
        return itr_err_fmt!(
            InstInvalid,
            "irnode {} return value check failed",
            inst as u8
        );
    }
    Ok(irnode)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn irlist_allows_noret_when_not_required() {
        // IRLIST: [P0, LOG1(P0, P1)] is a valid statement sequence (LOG1 consumes topic0 + data and returns no value).
        let bytes: Vec<u8> = vec![
            IRLIST as u8,
            0x00,
            0x02,
            P0 as u8,
            LOG1 as u8,
            P0 as u8,
            P1 as u8,
        ];
        let mut seek = 0usize;
        let node = parse_ir_node_one(&bytes, &mut seek).expect("parse IRLIST");
        assert!(!node.hasretval());
        assert_eq!(seek, bytes.len());
    }

    #[test]
    fn irlist_must_return_value_in_value_context() {
        // RET requires its child to produce a value. If we put an IRLIST that ends with LOG1(P0, P1) (no retval), parsing must fail.
        let bytes: Vec<u8> = vec![
            RET as u8,
            IRLIST as u8,
            0x00,
            0x01,
            LOG1 as u8,
            P0 as u8,
            P1 as u8,
        ];
        let mut seek = 0usize;
        assert!(parse_ir_node_one(&bytes, &mut seek).is_err());
    }

    #[test]
    fn dupn_is_rejected_by_irnode_parser() {
        let bytes: Vec<u8> = vec![DUPN as u8, 2];
        let mut seek = 0usize;
        let err = parse_ir_node_one(&bytes, &mut seek)
            .unwrap_err()
            .to_string();
        assert!(err.contains("dynamic multi-output"), "{}", err);
    }

    #[test]
    fn log_metadata_matches_runtime_arity() {
        let cases = [
            (LOG1, 2usize),
            (LOG2, 3usize),
            (LOG3, 4usize),
            (LOG4, 5usize),
        ];
        for (inst, argc) in cases {
            assert_eq!(inst.metadata().input as usize, argc);
            let mut bytes = vec![inst as u8];
            bytes.extend(std::iter::repeat(P0 as u8).take(argc));
            let mut seek = 0usize;
            let parsed = parse_ir_node_one(&bytes, &mut seek).unwrap();
            assert_eq!(seek, bytes.len());
            assert_eq!(parsed.subs(), argc);
            assert!(!parsed.hasretval());
            assert_eq!(parsed.codegen().unwrap(), {
                let mut out = vec![P0 as u8; argc];
                out.push(inst as u8);
                out
            });
        }
    }

    #[test]
    fn fin_ir_serializes_without_irlist_wrapper() {
        let n = IRNodeParam1Triple {
            hrtv: true,
            inst: Bytecode::FIN3,
            para: fin_source_call_spec("mul_div_floor").unwrap().unwrap().id,
            subx: push_inst(Bytecode::P1),
            suby: push_inst(Bytecode::P2),
            subz: push_inst(Bytecode::P3),
        };
        let bytes = n.serialize();
        assert_eq!(bytes[0], Bytecode::FIN3 as u8);
        let mut seek = 0usize;
        let parsed = parse_ir_node_one(&bytes, &mut seek).unwrap();
        assert_eq!(seek, bytes.len());
        assert!(parsed
            .as_any()
            .downcast_ref::<IRNodeParam1Triple>()
            .is_some());
    }

    #[test]
    fn malformed_fin_ir_is_rejected() {
        let bytes = vec![
            Bytecode::FIN4 as u8,
            fin_source_call_spec("mul_add_div_floor").unwrap().unwrap().id,
            Bytecode::P1 as u8,
        ];
        let mut seek = 0usize;
        assert!(parse_ir_node_one(&bytes, &mut seek).is_err());
    }
}
