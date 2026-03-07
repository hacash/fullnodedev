

#[derive(Default, Debug, Clone, PartialEq, Eq)]
pub enum Value {
    #[default] Nil,          // type_id = 0
    Bool(bool),              //           1
    U8(u8),                  //           2
    U16(u16),                //           3
    U32(u32),                //           4
    U64(u64),                //           5
    U128(u128),              //           6
    // U256(u256), ...       //           7..
    Bytes(Vec<u8>),          //           10
    Address(field::Address), //           11
    Args(ArgsItem),          //           12
    // ...                   //           ..
    HeapSlice((u32, u32)),   //           14
    Compo(CompoItem),        //           15
}


impl std::fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.to_string())
    }
}


use std::usize;

use Value::*;

impl Value {

    pub fn ty(&self) -> ValueTy {
        match self {
            Nil           => ValueTy::Nil,
            Bool(..)      => ValueTy::Bool,
            U8(..)        => ValueTy::U8,
            U16(..)       => ValueTy::U16,
            U32(..)       => ValueTy::U32,
            U64(..)       => ValueTy::U64,
            U128(..)      => ValueTy::U128,
            Bytes(..)     => ValueTy::Bytes,
            Address(..)   => ValueTy::Address,
            Args(..)      => ValueTy::Args,
            HeapSlice(..) => ValueTy::HeapSlice,
            Compo(..)     => ValueTy::Compo,
        }
    }

    pub fn nil() -> Self {
        Nil
    }

    pub fn bool(b: bool) -> Self {
        Bool(b)
    }

    pub fn u8(n: u8) -> Self {
        U8(n)
    }

    pub fn empty_bytes() -> Self {
        Bytes(vec![])
    }
    
    pub fn bytes(b: Vec<u8>) -> Self {
        Bytes(b)
    }

    pub fn is_nil(&self) -> bool {
        match self {
            Nil => true,
            _ => false,
        }
    }
    
    pub fn is_bool(&self) -> bool {
        match self {
            Bool(..) => true,
            _ => false,
        }
    }


    pub fn is_uint(&self) -> bool {
        self.ty().is_uint()
    }

    pub fn is_bytes(&self) -> bool {
        match self {
            Bytes(..) => true,
            _ => false,
        }
    }

    pub fn is_addr(&self) -> bool {
        match self {
            Address(..) => true,
            _ => false,
        }
    }

    pub fn match_bytes(&self) -> Option<&[u8]> {
        match self {
            Bytes(buf) => Some(buf.as_slice()),
            _ => None,
        }
    }

    pub fn match_bytes_mut(&mut self) -> Option<&mut Vec<u8>> {
        match self {
            Bytes(buf) => Some(buf),
            _ => None,
        }
    }

    pub fn match_compo(&self) -> Option<&CompoItem> {
        match self {
            Compo(compo) => Some(compo),
            _ => None,
        }
    }

    pub fn match_compo_mut(&mut self) -> Option<&mut CompoItem> {
        match self {
            Compo(compo) => Some(compo),
            _ => None,
        }
    }

    pub fn match_args(&self) -> Option<&ArgsItem> {
        match self {
            Args(args) => Some(args),
            _ => None,
        }
    }

    pub fn clone_argv_items(&self) -> VmrtRes<Vec<Value>> {
        if let Some(args) = self.match_args() {
            return Ok(args.to_vec())
        }
        if let Some(compo) = self.match_compo() {
            return Ok(compo.list_ref()?.iter().cloned().collect())
        }
        itr_err_code!(CompoOpNotMatch)
    }

    pub fn into_compo(self) -> Option<CompoItem> {
        match self {
            Compo(compo) => Some(compo),
            _ => None,
        }
    }

    pub fn compo_ref(&self) -> VmrtRes<&CompoItem> {
        let Some(compo) = self.match_compo() else {
            return itr_err_code!(CompoOpNotMatch)
        };
        Ok(compo)
    }

    pub fn compo(&mut self) -> VmrtRes<&mut CompoItem> {
        let Some(compo) = self.match_compo_mut() else {
            return itr_err_code!(CompoOpNotMatch)
        };
        Ok(compo)
    }

    pub fn take_compo(self) -> VmrtRes<CompoItem> {
        let Some(compo) = self.into_compo() else {
            return itr_err_code!(CompoOpNotMatch)
        };
        Ok(compo)
    }

    /* pub fn _____deval(&self, heap: &Heap) -> VmrtRes<Vec<u8>> { match self { Compo(..) => itr_err_code!(CompoToSerialize), HeapSlice((s, l)) => { match heap.do_read(*s as usize, *l as usize)? { Bytes(buf) => Ok(buf), _ => never!() } } _ => Ok(self.raw()) } } */


    pub fn raw(&self) -> Vec<u8> {
        match &self {
            Nil => vec![],
            Bool(n) => vec![maybe!(n, 1, 0)],
            U8(n) =>   n.to_be_bytes().into(),
            U16(n) =>  n.to_be_bytes().into(),
            U32(n) =>  n.to_be_bytes().into(),
            U64(n) =>  n.to_be_bytes().into(),
            U128(n) => n.to_be_bytes().into(),
            Bytes(buf) => buf.clone(),
            Address(a) => a.serialize(),
            Args(..) => "{args value ...}".to_owned().into_bytes(),
            HeapSlice((s, l)) => vec![s.to_be_bytes(), l.to_be_bytes()].concat(),
            // not support
            Compo(..) => "{compo value ...}".to_owned().into_bytes(),
        }
    }


    pub fn ty_num(&self) -> u8 {
        self.ty() as u8
    }

    pub fn val_size(&self) -> usize {
        match self {
            Nil      => 0,
            Bool(..) => 1,
            U8(..)   => 1,
            U16(..)  => 2,
            U32(..)  => 4,
            U64(..)  => 8,
            U128(..) => 16,
            Bytes(b) => b.len(),
            Address(..) => field::Address::SIZE,
            Args(a) => a.val_size(),
            HeapSlice((_, n)) => *n as usize,
            Compo(c) => c.val_size(),
        }
    }

    pub fn can_get_size(&self) -> VmrtRes<u16> {
        if let Args(..) | Compo(..) | HeapSlice(..) = self {
            return itr_err_code!(ItemNoSize)
        }
        let n = self.val_size();
        if n >= u16::MAX as usize {
            return itr_err_code!(OutOfValueSize)
        }
        Ok(n as u16)
    }

    pub fn valid(self, cap: &SpaceCap) -> VmrtRes<Self> {
        let cansz = self.can_get_size();
        match cansz {
            Ok(n) if n as usize > cap.max_value_size
                => return itr_err_code!(OutOfValueSize),
            _ => {}
        };
        Ok(self)
    }

    pub fn to_string(&self) -> String {
        match self {
            Nil =>          s!("nil"),
            Bool(true) =>   s!("true"),
            Bool(false) =>  s!("false"),
            U8(n) =>   format!("{}u8", n),
            U16(n) =>  format!("{}u16", n),
            U32(n) =>  format!("{}u32", n),
            U64(n) =>  format!("{}u64", n),
            U128(n) => format!("{}u128", n),
            Bytes(b) => match ascii_show_string(b) {
                Some(s) => format!("\"{}\"", s),
                _ => "0x".to_owned() + &hex::encode(b),
            },
            Address(a) => a.to_string(),
            Args(a) => a.to_string(),
            HeapSlice((s, l)) => format!("heap({},{})", s, l),
            Compo(a) => format!("compo({}){}", a.len(), a.to_string()),
        }
    }


    pub fn to_json(&self) -> String {
        match self {
            Nil =>          s!("null"),
            Bool(true) =>   s!("true"),
            Bool(false) =>  s!("false"),
            U8(n) =>   format!("{}", n),
            U16(n) =>  format!("{}", n),
            U32(n) =>  format!("{}", n),
            U64(n) =>  format!("{}", n),
            U128(n) => format!("{}", n),
            Bytes(b) => format!("\"{}\"", &to_readable_or_base64(b)),
            Address(a) =>  format!("\"{}\"", a),
            Args(a) => a.to_json(),
            HeapSlice((s, l)) => format!("[{},{}]", s, l),
            Compo(a) => a.to_json(),
        }
    }

    pub fn to_debug_json(&self) -> String {
        match self {
            Nil => s!("null"),
            Bool(true) => s!("true"),
            Bool(false) => s!("false"),
            U8(n) => format!("{}", n),
            U16(n) => format!("{}", n),
            U32(n) => format!("{}", n),
            U64(n) => format!("{}", n),
            U128(n) => format!("{}", n),
            Bytes(b) => match bytes_try_to_readable_string(b) {
                Some(s) => serde_json::to_string(&s).unwrap(),
                None => format!(r#"{{"$bytes_hex":"{}"}}"#, b.to_hex()),
            },
            Address(a) => serde_json::to_string(&a.to_string()).unwrap(),
            Args(a) => a.to_debug_json(),
            HeapSlice((s, l)) => format!(r#"{{"$heap":[{},{}]}}"#, s, l),
            Compo(a) => a.to_debug_json(),
        }
    }


}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rt::{ItrErr, ItrErrCode};
    use std::collections::{BTreeMap, VecDeque};

    #[test]
    fn can_get_size_returns_error_instead_of_panicking_on_u16_max() {
        let v = Value::Bytes(vec![0u8; u16::MAX as usize]);
        assert!(matches!(v.can_get_size(), Err(ItrErr(ItrErrCode::OutOfValueSize, _))));
    }

    #[test]
    fn can_get_size_allows_u16_max_minus_one() {
        let v = Value::Bytes(vec![0u8; (u16::MAX as usize) - 1]);
        assert_eq!(v.can_get_size().unwrap(), u16::MAX - 1);
    }

    #[test]
    fn val_size_counts_compo_list_and_map() {
        let list = CompoItem::list(VecDeque::from([
            Value::U8(7),
            Value::Address(field::Address::default()),
            Value::Bytes(vec![1, 2, 3]),
        ])).unwrap();
        let listv = Value::Compo(list);
        assert_eq!(listv.val_size(), 1 + field::Address::SIZE + 3);

        let mut map = BTreeMap::new();
        map.insert(vec![0u8; 2], Value::U16(9));
        map.insert(vec![1u8; 4], Value::Bytes(vec![5u8; 3]));
        let mapv = Value::Compo(CompoItem::map(map).unwrap());
        assert_eq!(mapv.val_size(), (2 + 2) + (4 + 3));
    }

    #[test]
    fn match_bytes_and_compo_helpers_work() {
        let mut bytes = Value::Bytes(vec![1, 2, 3]);
        assert_eq!(bytes.match_bytes(), Some(&[1, 2, 3][..]));
        bytes.match_bytes_mut().unwrap().push(4);
        assert_eq!(bytes.match_bytes(), Some(&[1, 2, 3, 4][..]));
        assert!(bytes.match_compo().is_none());
        assert!(bytes.match_args().is_none());

        let mut compo = Value::Compo(CompoItem::new_list());
        assert!(compo.match_compo().is_some());
        assert!(compo.match_compo_mut().is_some());
        assert!(compo.match_bytes().is_none());

        let args = Value::Args(ArgsItem::new(vec![Value::U8(1), Value::U16(2)]).unwrap());
        assert!(args.match_args().is_some());
        assert_eq!(args.clone_argv_items().unwrap(), vec![Value::U8(1), Value::U16(2)]);
    }

    #[test]
    fn to_debug_json_escapes_readable_bytes() {
        let v = Value::Bytes(br#"a"b\c"#.to_vec());
        assert_eq!(v.to_debug_json(), r#""a\"b\\c""#);
    }

    #[test]
    fn to_debug_json_keeps_readable_map_as_plain_object() {
        let mut map = BTreeMap::new();
        map.insert(b"kind".to_vec(), Value::Bytes(b"hnft".to_vec()));
        map.insert(b"mint".to_vec(), Value::U8(1));
        let v = Value::Compo(CompoItem::map(map).unwrap());
        assert_eq!(v.to_debug_json(), r#"{"kind":"hnft","mint":1}"#);
    }

    #[test]
    fn to_debug_json_falls_back_for_binary_map_keys() {
        let mut map = BTreeMap::new();
        map.insert(vec![0x00, 0xff], Value::U8(7));
        let v = Value::Compo(CompoItem::map(map).unwrap());
        assert_eq!(v.to_debug_json(), r#"{"$map":[{"key_hex":"00ff","value":7}]}"#);
    }
}
