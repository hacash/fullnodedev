fn cannot_cast_err(v: &Value, ty: &str) -> ItrErr {
    ItrErr::new(CastFail, &format!("cannot cast {:?} to {}", v, ty))
}

fn cast_uint_name(bits: u16) -> &'static str {
    match bits {
        8 => "U8",
        16 => "U16",
        32 => "U32",
        64 => "U64",
        128 => "U128",
        256 => "U256",
        _ => "UINT",
    }
}

fn ensure_active_uint_bits(bits: u16) -> VmrtErr {
    if bits == RESERVED_U256_BITS {
        return itr_err_fmt!(CastFail, "U256 is reserved but not enabled");
    }
    if ACTIVE_UINT_BITS.contains(&bits) {
        return Ok(());
    }
    itr_err_code!(CastFail)
}

fn bytes_width_err(buf: &[u8], bits: u16) -> ItrErr {
    ItrErr::new(
        CastFail,
        &format!(
            "cannot cast {:?} to {}",
            Value::Bytes(buf.to_vec()),
            cast_uint_name(bits)
        ),
    )
}

fn bytes_to_fixed_width<const N: usize>(buf: &[u8], bits: u16) -> VmrtRes<[u8; N]> {
    if buf.len() <= N {
        let mut out = [0u8; N];
        out[N - buf.len()..].copy_from_slice(buf);
        return Ok(out);
    }
    let cut = buf.len() - N;
    if buf[..cut].iter().any(|b| *b != 0) {
        return Err(bytes_width_err(buf, bits));
    }
    let mut out = [0u8; N];
    out.copy_from_slice(&buf[cut..]);
    Ok(out)
}

fn bytes_to_uint_width(buf: &[u8], bits: u16) -> VmrtRes<Value> {
    ensure_active_uint_bits(bits)?;
    Ok(match bits {
        8 => Value::U8(u8::from_be_bytes(bytes_to_fixed_width::<1>(buf, bits)?)),
        16 => Value::U16(u16::from_be_bytes(bytes_to_fixed_width::<2>(buf, bits)?)),
        32 => Value::U32(u32::from_be_bytes(bytes_to_fixed_width::<4>(buf, bits)?)),
        64 => Value::U64(u64::from_be_bytes(bytes_to_fixed_width::<8>(buf, bits)?)),
        128 => Value::U128(u128::from_be_bytes(bytes_to_fixed_width::<16>(buf, bits)?)),
        _ => return itr_err_code!(CastFail),
    })
}

fn arith_uint_bits(v: &Value) -> Option<u16> {
    v.ty().uint_bits()
}

pub fn cast_arithmetic(x: &mut Value, y: &mut Value) -> VmrtErr {
    let (Some(lb), Some(rb)) = (arith_uint_bits(x), arith_uint_bits(y)) else {
        return itr_err_fmt!(
            CastFail,
            "cannot do arithmetic cast between type {:?} and {:?}",
            x,
            y
        );
    };
    let tb = lb.max(rb);
    if lb < tb {
        x.cast_to_uint_width(tb)?;
    }
    if rb < tb {
        y.cast_to_uint_width(tb)?;
    }
    Ok(())
}

impl Value {
    pub fn cast_bool(&mut self) -> VmrtErr {
        *self = self.to_bool()?;
        Ok(())
    }

    pub fn cast_bool_not(&mut self) -> VmrtErr {
        self.cast_bool()?;
        let Bool(b) = self else { never!() };
        *b = !*b;
        Ok(())
    }

    pub fn cast_to_uint_width(&mut self, bits: u16) -> VmrtErr {
        ensure_active_uint_bits(bits)?;
        let name = cast_uint_name(bits);
        if let Bytes(buf) = self {
            *self = bytes_to_uint_width(buf, bits)?;
            return Ok(());
        }
        let v = self.canbe_u128().map_err(|_| cannot_cast_err(self, name))?;
        *self = match bits {
            8 => Value::U8(u8::try_from(v).map_err(|_| cannot_cast_err(self, name))?),
            16 => Value::U16(u16::try_from(v).map_err(|_| cannot_cast_err(self, name))?),
            32 => Value::U32(u32::try_from(v).map_err(|_| cannot_cast_err(self, name))?),
            64 => Value::U64(u64::try_from(v).map_err(|_| cannot_cast_err(self, name))?),
            128 => Value::U128(v),
            _ => return itr_err_code!(CastFail),
        };
        Ok(())
    }

    pub fn cast_u8(&mut self) -> VmrtErr {
        self.cast_to_uint_width(8)
    }

    pub fn cast_u16(&mut self) -> VmrtErr {
        self.cast_to_uint_width(16)
    }

    pub fn cast_u32(&mut self) -> VmrtErr {
        self.cast_to_uint_width(32)
    }

    pub fn cast_u64(&mut self) -> VmrtErr {
        self.cast_to_uint_width(64)
    }

    pub fn cast_u128(&mut self) -> VmrtErr {
        self.cast_to_uint_width(128)
    }

    pub fn cast_buf(&mut self) -> VmrtErr {
        if matches!(self, Bytes(..)) {
            return Ok(());
        }
        *self = Bytes(self.canbe_bytes_ec(CastFail)?);
        Ok(())
    }

    pub fn cast_addr(&mut self) -> VmrtErr {
        if matches!(self, Address(..)) {
            return Ok(());
        }
        let Bytes(buf) = self else {
            return itr_err_fmt!(CastFail, "cannot cast {:?} to address", self);
        };
        let adr = field::Address::from_bytes(buf).map_ire(CastFail)?;
        *self = Address(adr);
        Ok(())
    }

    fn cast_to_ty(&mut self, ty: ValueTy) -> VmrtErr {
        use ValueTy::*;
        match ty {
            Bool => self.cast_bool(),
            U8 => self.cast_u8(),
            U16 => self.cast_u16(),
            U32 => self.cast_u32(),
            U64 => self.cast_u64(),
            U128 => self.cast_u128(),
            Bytes => self.cast_buf(),
            Address => self.cast_addr(),
            _ => itr_err_code!(CastFail),
        }
    }

    pub fn cast_to(&mut self, ty: u8) -> VmrtErr {
        let ty = ValueTy::build(ty).map_ire(CastFail)?;
        self.cast_to_ty(ty)
    }

    fn fn_boundary_type_fail(expect: ValueTy, actual: ValueTy) -> ItrErr {
        ItrErr::new(
            CallArgvTypeFail,
            &format!("expected {:?} but got {:?}", expect, actual),
        )
    }

    pub fn cast_param(&mut self, ty: ValueTy) -> VmrtErr {
        let actual = self.ty();
        if ty == actual {
            return Ok(());
        }
        if ty.is_uint() && actual.is_uint() {
            return self.cast_to_ty(ty).map_err(|ItrErr(_, msg)| {
                if msg.is_empty() {
                    Self::fn_boundary_type_fail(ty, actual)
                } else {
                    ItrErr::new(CallArgvTypeFail, &msg)
                }
            });
        }
        Err(Self::fn_boundary_type_fail(ty, actual))
    }

    pub fn checked_param_type(&self, ty: ValueTy) -> VmrtErr {
        let mut tmp = self.clone();
        tmp.cast_param(ty)
    }
}

#[cfg(test)]
mod cast_tests {
    use super::*;

    #[test]
    fn cast_param_allows_narrowing_uint() {
        let mut v = Value::U32(1);
        v.cast_param(ValueTy::U16).unwrap();
        assert_eq!(v, Value::U16(1));
    }

    #[test]
    fn cast_param_allows_widening_uint() {
        let mut v = Value::U16(7);
        v.cast_param(ValueTy::U64).unwrap();
        assert_eq!(v, Value::U64(7));
    }

    #[test]
    fn cast_param_rejects_cross_family_casts() {
        let mut v = Value::U8(0);
        let err = v.cast_param(ValueTy::Bool).unwrap_err();
        assert_eq!(err.0, ItrErrCode::CallArgvTypeFail);
        assert_eq!(v, Value::U8(0));
    }

    #[test]
    fn cast_param_invalid_target_uses_call_argv_type_fail() {
        let mut v = Value::U8(1);
        let err = v.cast_param(ValueTy::Compo).unwrap_err();
        assert_eq!(err.0, ItrErrCode::CallArgvTypeFail);
    }

    #[test]
    fn checked_param_type_uses_uint_boundary_rules() {
        let ok = Value::U32(1);
        assert!(ok.checked_param_type(ValueTy::U16).is_ok());

        let overflow = Value::U32(70000);
        assert!(overflow.checked_param_type(ValueTy::U16).is_err());

        let bytes = Value::Bytes(vec![1]);
        assert!(bytes.checked_param_type(ValueTy::U8).is_err());
    }

    #[test]
    fn cast_uint_width_accepts_bool_and_nil() {
        let mut b = Value::Bool(true);
        b.cast_u8().unwrap();
        assert_eq!(b, Value::U8(1));

        let mut n = Value::Nil;
        n.cast_u16().unwrap();
        assert_eq!(n, Value::U16(0));
    }
}
