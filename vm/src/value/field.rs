

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
                (1, Bool(maybe!(b == 0, false, true)))
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
            _ => return errf!("Compo or slice value item cannot be parse"),
        };
        Ok(sz + 1)
    }
}

impl Serialize for Value {
    fn serialize(&self) -> Vec<u8> {
        let ty = self.ty_num();
        let mut buf = self.raw();
        if self.is_bytes() { // Uint
            buf = [(buf.len() as u16).to_be_bytes().to_vec(), buf].concat()
        }
        iter::once(ty).chain(buf).collect()
    }

    fn size(&self) -> usize {
        // Keep size() panic-free for non-storable variants (Compo/HeapSlice).
        // This matches the serialized length contract directly.
        if self.is_bytes() {
            let base = self.raw().len();
            return 1 + 2 + base // type_id + bytes len prefix + payload
        }
        1 + self.raw().len() // type_id + payload
    }
}


impl Field for Value {}

impl ToJSON for Value {
    fn to_json_fmt(&self, _fmt: &JSONFormater) -> String {
        format!("\"{}\"", self.to_string())
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

    #[test]
    fn value_size_is_panic_free_for_non_storable_variants() {
        let hv = Value::HeapSlice((3, 7));
        assert_eq!(<Value as Serialize>::size(&hv), 1 + 8);

        let compo = CompoItem::list(VecDeque::from([Value::U8(1), Value::U16(2)])).unwrap();
        let cv = Value::Compo(compo);
        let sz = <Value as Serialize>::size(&cv);
        assert!(sz > 1);
    }
}
