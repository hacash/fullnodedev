#[allow(dead_code)]
impl Syntax {
    fn parse_slot_str(id: &str) -> Option<u8> {
        if start_with_char(id, '$') {
            if let Ok(idx) = id.trim_start_matches('$').parse::<u8>() {
                return Some(idx);
            }
        }
        None
    }

    fn parse_scalar_value_ty(token: &Token) -> Option<ValueTy> {
        match token {
            Keyword(KwTy::Bool) => Some(ValueTy::Bool),
            Keyword(KwTy::U8) => Some(ValueTy::U8),
            Keyword(KwTy::U16) => Some(ValueTy::U16),
            Keyword(KwTy::U32) => Some(ValueTy::U32),
            Keyword(KwTy::U64) => Some(ValueTy::U64),
            Keyword(KwTy::U128) => Some(ValueTy::U128),
            Keyword(KwTy::Bytes) => Some(ValueTy::Bytes),
            Keyword(KwTy::Address) => Some(ValueTy::Address),
            _ => None,
        }
    }

    fn parse_uint_suffix_cast(token: &Token) -> Option<(ValueTy, Bytecode)> {
        match token {
            Keyword(KwTy::U8) => Some((ValueTy::U8, Bytecode::CU8)),
            Keyword(KwTy::U16) => Some((ValueTy::U16, Bytecode::CU16)),
            Keyword(KwTy::U32) => Some((ValueTy::U32, Bytecode::CU32)),
            Keyword(KwTy::U64) => Some((ValueTy::U64, Bytecode::CU64)),
            Keyword(KwTy::U128) => Some((ValueTy::U128, Bytecode::CU128)),
            _ => None,
        }
    }

    fn build_cast_node(left: Box<dyn IRNode>, ty: ValueTy) -> Box<dyn IRNode> {
        match ty {
            ValueTy::Bool | ValueTy::Address => push_single_p1_hr(true, Bytecode::CTO, ty as u8, left),
            ValueTy::U8 => push_single(Bytecode::CU8, left),
            ValueTy::U16 => push_single(Bytecode::CU16, left),
            ValueTy::U32 => push_single(Bytecode::CU32, left),
            ValueTy::U64 => push_single(Bytecode::CU64, left),
            ValueTy::U128 => push_single(Bytecode::CU128, left),
            ValueTy::Bytes => push_single(Bytecode::CBUF, left),
            _ => never!(),
        }
    }

    fn build_is_node(subx: Box<dyn IRNode>, ty: ValueTy) -> Box<dyn IRNode> {
        match ty {
            ValueTy::Nil => push_single(Bytecode::TNIL, subx),
            _ => push_single_p1_hr(true, Bytecode::TIS, ty as u8, subx),
        }
    }

    fn literal_value_type(node: &dyn IRNode) -> Option<ValueTy> {
        use Bytecode::*;
        if let Some(leaf) = node.as_any().downcast_ref::<IRNodeLeaf>() {
            return match leaf.inst {
                P0 | P1 | P2 | P3 => Some(ValueTy::U8),
                PTRUE | PFALSE => Some(ValueTy::Bool),
                PNBUF => Some(ValueTy::Bytes),
                _ => None,
            };
        }
        if let Some(param1) = node.as_any().downcast_ref::<IRNodeParam1>() {
            if param1.inst == PU8 {
                return Some(ValueTy::U8);
            }
        }
        if let Some(param2) = node.as_any().downcast_ref::<IRNodeParam2>() {
            if param2.inst == PU16 {
                return Some(ValueTy::U16);
            }
        }
        if let Some(params) = node.as_any().downcast_ref::<IRNodeParams>() {
            return match params.inst {
                PBUF | PBUFL => Some(ValueTy::Bytes),
                _ => None,
            };
        }
        if let Some(single) = node.as_any().downcast_ref::<IRNodeSingle>() {
            return match single.inst {
                CU8 => Some(ValueTy::U8),
                CU16 => Some(ValueTy::U16),
                CU32 => Some(ValueTy::U32),
                CU64 => Some(ValueTy::U64),
                CU128 => Some(ValueTy::U128),
                CBUF => Some(ValueTy::Bytes),
                _ => None,
            };
        }
        if let Some(single) = node.as_any().downcast_ref::<IRNodeParam1Single>() {
            if single.inst == CTO {
                return ValueTy::build(single.para).ok();
            }
        }
        None
    }

    fn params_literal_bytes(params: &IRNodeParams) -> Option<Vec<u8>> {
        use Bytecode::*;
        let header_len = match params.inst {
            PBUF => 1,
            PBUFL => 2,
            _ => return None,
        };
        if params.para.len() < header_len {
            return None;
        }
        let data_len = match header_len {
            1 => params.para[0] as usize,
            2 => u16::from_be_bytes([params.para[0], params.para[1]]) as usize,
            _ => never!(),
        };
        if params.para.len() != header_len + data_len {
            return None;
        }
        Some(params.para[header_len..].to_vec())
    }

    fn extract_literal_value(node: &dyn IRNode) -> Ret<Option<Value>> {
        use Bytecode::*;
        if let Some(leaf) = node.as_any().downcast_ref::<IRNodeLeaf>() {
            let v = match leaf.inst {
                P0 => Value::U8(0),
                P1 => Value::U8(1),
                P2 => Value::U8(2),
                P3 => Value::U8(3),
                PNIL => Value::Nil,
                PTRUE => Value::Bool(true),
                PFALSE => Value::Bool(false),
                PNBUF => Value::Bytes(vec![]),
                _ => return Ok(None),
            };
            return Ok(Some(v));
        }
        if let Some(param1) = node.as_any().downcast_ref::<IRNodeParam1>() {
            if param1.inst == PU8 {
                return Ok(Some(Value::U8(param1.para)));
            }
            return Ok(None);
        }
        if let Some(param2) = node.as_any().downcast_ref::<IRNodeParam2>() {
            if param2.inst == PU16 {
                return Ok(Some(Value::U16(u16::from_be_bytes(param2.para))));
            }
            return Ok(None);
        }
        if let Some(params) = node.as_any().downcast_ref::<IRNodeParams>() {
            return Ok(Self::params_literal_bytes(params).map(Value::Bytes));
        }
        if let Some(single) = node.as_any().downcast_ref::<IRNodeSingle>() {
            let Some(mut v) = Self::extract_literal_value(&*single.subx)? else {
                return Ok(None);
            };
            let cast_res = match single.inst {
                CU8 => v.cast_u8(),
                CU16 => v.cast_u16(),
                CU32 => v.cast_u32(),
                CU64 => v.cast_u64(),
                CU128 => v.cast_u128(),
                CBUF => v.cast_buf(),
                _ => return Ok(None),
            };
            if cast_res.is_err() {
                return Ok(None);
            }
            return Ok(Some(v));
        }
        if let Some(single) = node.as_any().downcast_ref::<IRNodeParam1Single>() {
            if single.inst != CTO {
                return Ok(None);
            }
            let Some(mut v) = Self::extract_literal_value(&*single.subx)? else {
                return Ok(None);
            };
            if v.cast_to(single.para).is_err() {
                return Ok(None);
            }
            return Ok(Some(v));
        }
        Ok(None)
    }

    fn check_uint_literal_overflow(n: u128, ty: ValueTy) -> Rerr {
        match ty {
            ValueTy::U8 => {
                if n > u8::MAX as u128 {
                    return errf!("integer {} overflows u8 (max: {})", n, u8::MAX);
                }
            }
            ValueTy::U16 => {
                if n > u16::MAX as u128 {
                    return errf!("integer {} overflows u16 (max: {})", n, u16::MAX);
                }
            }
            ValueTy::U32 => {
                if n > u32::MAX as u128 {
                    return errf!("integer {} overflows u32 (max: {})", n, u32::MAX);
                }
            }
            ValueTy::U64 => {
                if n > u64::MAX as u128 {
                    return errf!("integer {} overflows u64 (max: {})", n, u64::MAX);
                }
            }
            ValueTy::U128 => {}
            _ => {}
        }
        Ok(())
    }

    fn check_literal_as_cast(node: &dyn IRNode, target_ty: ValueTy) -> Rerr {
        let Some(mut literal) = Self::extract_literal_value(node)? else {
            return Ok(());
        };

        if literal.ty().is_uint() && target_ty.is_uint() {
            let n = literal.extract_u128()?;
            Self::check_uint_literal_overflow(n, target_ty)?;
        }

        match target_ty {
            ValueTy::Bool => literal.cast_bool()?,
            ValueTy::U8 => literal.cast_u8()?,
            ValueTy::U16 => literal.cast_u16()?,
            ValueTy::U32 => literal.cast_u32()?,
            ValueTy::U64 => literal.cast_u64()?,
            ValueTy::U128 => literal.cast_u128()?,
            ValueTy::Bytes => literal.cast_buf()?,
            ValueTy::Address => literal.cast_addr()?,
            _ => {}
        }
        Ok(())
    }
}
