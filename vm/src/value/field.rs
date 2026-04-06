

#[derive(Default, Debug, Clone)]
pub struct ValueKey {
    bytes: Vec<u8>
} 

impl Parse for ValueKey {
    fn parse(&mut self, buf: &[u8]) -> Ret<usize> {
        self.bytes = buf.to_vec();
        Ok(buf.len())
    }
}

impl Serialize for ValueKey {
    fn serialize(&self) -> Vec<u8> {
        self.bytes.clone()
    }
    fn size(&self) -> usize {
        self.bytes.len()
    }
}

impl Field for ValueKey {}

impl ToJSON for ValueKey {
    fn to_json_fmt(&self, _fmt: &JSONFormater) -> String {
        format!("\"0x{}\"", hex::encode(&self.bytes))
    }
}
impl FromJSON for ValueKey {
    fn from_json(&mut self, json: &str) -> Ret<()> {
        self.bytes = field::json_decode_binary(json)?;
        Ok(())
    }
}

impl ValueKey {
    pub fn from(bytes: Vec<u8>) -> Self {
        Self { bytes }
    }
}



/*************************/

// just for storage

impl Parse for Value {
    fn parse(&mut self, mut buf: &[u8]) -> Ret<usize>{
        let err = errf!("value buf too short");
        let bl = buf.len();
        if bl < 1 {
            return err
        }
        let ty = ValueTy::build(buf[0])?;
        buf = &buf[1..];
        macro_rules! buf_to_uint { ($ty:ty, $buf:expr, $l:expr) => {{
            if buf.len() < $l {
                return err
            }
            <$ty>::from_be_bytes(buf[0..$l].try_into().unwrap())
        }}}
        let sz: usize;
        (sz, *self) = match ty {
            ValueTy::Nil     => (0, Nil),
            ValueTy::Bool    => {
                let b = buf_to_uint!(u8, buf, 1);
                let Some(b) = decode_canonical_bool_byte(b) else {
                    return errf!("value bool invalid")
                };
                (1, Bool(b))
            },
            ValueTy::U8      => (1, U8(buf_to_uint!(u8, buf, 1))),
            ValueTy::U16     => (2,   U16(buf_to_uint!(u16,  buf,  2))),
            ValueTy::U32     => (4,   U32(buf_to_uint!(u32,  buf,  4))),
            ValueTy::U64     => (8,   U64(buf_to_uint!(u64,  buf,  8))),
            ValueTy::U128    => (16, U128(buf_to_uint!(u128, buf, 16))),
            ValueTy::Bytes   => {
                let l = buf_to_uint!(u16,  buf,  2) as usize;
                buf = &buf[2..];
                if buf.len() < l {
                    return err
                }
                (2 + l as usize, Bytes(buf[0..l].to_vec()))
            },
            ValueTy::Address => {
                let (adr, sz) = field::Address::create(buf)?;
                (sz, Address(adr))
            },
            _ => return errf!("Tuple, handle, compo or slice value item cannot be parsed"),
        };
        Ok(sz + 1)
    }
}

impl Serialize for Value {
    fn serialize(&self) -> Vec<u8> {
        match self {
            // Runtime-only variants are intentionally excluded from field serialization.
            // Parse also rejects them, so serialize must keep the same type boundary.
            HeapSlice(..) | Tuple(..) | Handle(..) | Compo(..) => {
                panic!("Value::serialize does not support HeapSlice/Tuple/Handle/Compo")
            }
            Bytes(buf) => {
                assert!(
                    buf.len() < u16::MAX as usize,
                    "Value::serialize bytes length {} exceeds u16 field limit",
                    buf.len()
                );
                let mut out = Vec::with_capacity(1 + 2 + buf.len());
                out.push(self.ty_num());
                out.extend_from_slice(&(buf.len() as u16).to_be_bytes());
                out.extend_from_slice(buf);
                out
            }
            _ => {
                let buf = self.raw();
                let mut out = Vec::with_capacity(1 + buf.len());
                out.push(self.ty_num());
                out.extend_from_slice(&buf);
                out
            }
        }
    }

    fn size(&self) -> usize {
        match self {
            HeapSlice(..) | Tuple(..) | Handle(..) | Compo(..) => {
                panic!("Value::size does not support HeapSlice/Tuple/Handle/Compo")
            }
            Bytes(buf) => {
                assert!(
                    buf.len() < u16::MAX as usize,
                    "Value::size bytes length {} exceeds u16 field limit",
                    buf.len()
                );
                1 + 2 + buf.len()
            }
            _ => 1 + self.raw().len(),
        }
    }
}


impl Field for Value {}

impl ToJSON for Value {
    fn to_json_fmt(&self, _fmt: &JSONFormater) -> String {
        Value::to_json(self)
    }
}
impl FromJSON for Value {
    fn from_json(&mut self, _json: &str) -> Ret<()> {
        errf!("Value FromJSON not implemented")
    }
}

#[cfg(test)]
mod field_tests {
    use super::*;
    use std::collections::VecDeque;

    fn assert_panics<F>(f: F)
    where
        F: FnOnce(),
    {
        let res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(f));
        assert!(res.is_err());
    }

    #[test]
    fn value_size_matches_serialize_len_for_storable_variants() {
        let values = [
            Value::Nil,
            Value::Bool(true),
            Value::U8(7),
            Value::U16(9),
            Value::Bytes(vec![1, 2, 3]),
            Value::Address(field::Address::create_contract([1u8; 20])),
        ];
        for value in values {
            assert_eq!(<Value as Serialize>::size(&value), <Value as Serialize>::serialize(&value).len());
        }
    }

    #[test]
    fn value_size_panics_for_runtime_only_variants() {
        let hv = Value::HeapSlice((3, 7));
        assert_panics(|| {
            <Value as Serialize>::size(&hv);
        });

        let compo = CompoItem::list(VecDeque::from([Value::U8(1), Value::U16(2)])).unwrap();
        let cv = Value::Compo(compo);
        assert_panics(|| {
            <Value as Serialize>::size(&cv);
        });

        let av = Value::Tuple(TupleItem::new(vec![Value::U8(1), Value::U16(2)]).unwrap());
        assert_panics(|| {
            <Value as Serialize>::size(&av);
        });

        let hv = Value::handle(7u32);
        assert_panics(|| {
            <Value as Serialize>::size(&hv);
        });
    }

    #[test]
    fn value_serialize_panics_for_runtime_only_variants() {
        let hv = Value::HeapSlice((3, 7));
        assert_panics(|| {
            <Value as Serialize>::serialize(&hv);
        });

        let compo = CompoItem::list(VecDeque::from([Value::U8(1)])).unwrap();
        let cv = Value::Compo(compo);
        assert_panics(|| {
            <Value as Serialize>::serialize(&cv);
        });

        let av = Value::Tuple(TupleItem::new(vec![Value::U8(1)]).unwrap());
        assert_panics(|| {
            <Value as Serialize>::serialize(&av);
        });

        let hv = Value::handle(7u32);
        assert_panics(|| {
            <Value as Serialize>::serialize(&hv);
        });
    }

    #[test]
    fn value_serialize_enforces_u16_bytes_limit() {
        let ok = Value::Bytes(vec![0u8; (u16::MAX as usize) - 1]);
        let enc = <Value as Serialize>::serialize(&ok);
        assert_eq!(enc.len(), <Value as Serialize>::size(&ok));
        assert_eq!(u16::from_be_bytes([enc[1], enc[2]]) as usize, (u16::MAX as usize) - 1);

        let too_large = Value::Bytes(vec![0u8; u16::MAX as usize]);
        assert_panics(|| {
            <Value as Serialize>::serialize(&too_large);
        });
        assert_panics(|| {
            <Value as Serialize>::size(&too_large);
        });
    }

    #[test]
    fn value_tojson_trait_matches_value_json_semantics() {
        let values = [
            Value::Nil,
            Value::Bool(true),
            Value::U8(7),
            Value::Bytes(br#"a"b\c"#.to_vec()),
        ];
        for value in values {
            assert_eq!(value.to_json_fmt(&JSONFormater::default()), value.to_json());
        }
    }

    #[test]
    fn value_parse_bool_requires_canonical_byte() {
        let parse = |buf: &[u8]| -> Ret<Value> {
            let mut out = Value::Nil;
            let used = out.parse(buf)?;
            if used != buf.len() {
                return errf!("field parse not consumed all bytes");
            }
            Ok(out)
        };
        assert_eq!(parse(&[ValueTy::Bool as u8, 0]).unwrap(), Value::Bool(false));
        assert_eq!(parse(&[ValueTy::Bool as u8, 1]).unwrap(), Value::Bool(true));
        assert!(parse(&[ValueTy::Bool as u8, 2]).is_err());
    }
}
