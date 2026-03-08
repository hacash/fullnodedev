

impl Value {

    fn check_func_boundary(value: &Value, ec: ItrErrCode, allow_args: bool) -> VmrtErr {
        match value {
            HeapSlice(..) => itr_err_code!(ec),
            Args(args) => {
                if !allow_args {
                    return itr_err_code!(ec)
                }
                for item in args.as_slice() {
                    Self::check_func_boundary(item, ec, false)?;
                }
                Ok(())
            }
            Compo(compo) => {
                if let Ok(list) = compo.list_ref() {
                    for v in list {
                        Self::check_func_boundary(v, ec, false)?;
                    }
                    return Ok(())
                }
                if let Ok(map) = compo.map_ref() {
                    for v in map.values() {
                        Self::check_func_boundary(v, ec, false)?;
                    }
                    return Ok(())
                }
                itr_err_code!(ec)
            }
            _ => Ok(()),
        }
    }

    fn check_func_argv_item(value: &Value, ec: ItrErrCode) -> VmrtErr {
        Self::check_func_boundary(value, ec, false)
    }

    pub fn canbe_bytes_ec(&self, ec: ItrErrCode) -> VmrtRes<Vec<u8>> {
        match self {
            Bool(b) => Ok(vec![maybe!(b, 1, 0)]),
            U8(n) => Ok(n.to_be_bytes().into()),
            U16(n) => Ok(n.to_be_bytes().into()),
            U32(n) => Ok(n.to_be_bytes().into()),
            U64(n) => Ok(n.to_be_bytes().into()),
            U128(n) => Ok(n.to_be_bytes().into()),
            Bytes(b) => Ok(b.clone()),
            Address(a) => Ok(a.to_vec()),
            _ => itr_err_code!(ec),
        }
    }

    pub fn canbe_key(&self) -> VmrtRes<Vec<u8>> {
        let ec = CastBeKeyFail;
        match self {
            Bool(..) => itr_err_code!(ec),
            _ => self.canbe_bytes_ec(ec),
        }
    }

    pub fn canbe_value(&self) -> VmrtErr {
        let ec = CastBeValueFail;
        match self {
            Nil |
            Bool(..)|
            U8(..)    |
            U16(..)   |
            U32(..)   |
            U64(..)   |
            U128(..)  |
            Bytes(..) |
            Address(..) => Ok(()),
            _ => itr_err_code!(ec)
        }
    }

    pub fn canbe_call_data(&self, heap: &Heap) -> VmrtRes<Vec<u8>> {
        let ec = CastBeCallDataFail;
        match self {
            Nil => Ok(vec![]),
            HeapSlice((start, length)) => {
                let Value::Bytes(buf) = heap.do_read(*start as usize, *length as usize)? else {
                    never!()
                };
                Ok(buf)
            }
            _ => self.canbe_bytes_ec(ec),
        }
    }

    pub fn canbe_func_argv(&self) -> VmrtErr {
        Self::check_func_boundary(self, CastBeFnArgvFail, true)
    }

    pub fn canbe_func_retv(&self) -> VmrtErr {
        Self::check_func_boundary(self, CastBeFnRetvFail, true)
    }





}

#[cfg(test)]
mod canbe_tests {
    use super::*;

    #[test]
    fn heapslice_ext_call_data_reads_heap_bytes_only_here() {
        let mut heap = Heap::new(64);
        heap.grow(1).unwrap();
        heap.write(0, Value::Bytes(vec![1, 2, 3, 4])).unwrap();
        let hs = Value::HeapSlice((1, 2));

        assert_eq!(hs.canbe_call_data(&heap).unwrap(), vec![2, 3]);
        assert!(hs.canbe_bytes_ec(CastBeBytesFail).is_err());
        assert!(hs.canbe_func_argv().is_err());
        assert!(hs.canbe_func_retv().is_err());

        let args = Value::Args(ArgsItem::new(vec![Value::U8(1), Value::Compo(CompoItem::new_list())]).unwrap());
        assert!(args.canbe_func_argv().is_ok());
        assert!(args.canbe_func_retv().is_ok());

    }

    #[test]
    fn heapslice_func_retv_uses_retv_error_code() {
        let hs = Value::HeapSlice((0, 1));
        let err = hs.canbe_func_retv().unwrap_err();
        assert_eq!(err.0, ItrErrCode::CastBeFnRetvFail);
    }
}
