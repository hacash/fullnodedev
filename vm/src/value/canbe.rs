fn check_scalar_ec(value: &Value, ec: ItrErrCode) -> VmrtErr {
    match value {
        Nil | Bool(..) | U8(..) | U16(..) | U32(..) | U64(..) | U128(..) | Bytes(..)
        | Address(..) => Ok(()),
        _ => itr_err_code!(ec),
    }
}

fn check_tuple_item_ec(value: &Value, ec: ItrErrCode) -> VmrtErr {
    match value {
        HeapSlice(..) | Tuple(..) => itr_err_code!(ec),
        Compo(..) => Ok(()),
        _ => check_scalar_ec(value, ec),
    }
}

fn check_func_boundary_ec(value: &Value, ec: ItrErrCode) -> VmrtErr {
    match value {
        HeapSlice(..) => itr_err_code!(ec),
        Tuple(tuple) => {
            for item in tuple.as_slice() {
                check_tuple_item_ec(item, ec)?;
            }
            Ok(())
        }
        Compo(..) => Ok(()),
        _ => check_scalar_ec(value, ec),
    }
}

impl Value {
    fn extract_bytes_with_error_code(&self, ec: ItrErrCode) -> VmrtRes<Vec<u8>> {
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

    pub fn extract_bytes(&self) -> VmrtRes<Vec<u8>> {
        self.extract_bytes_with_error_code(CastBeBytesFail)
    }

    pub fn extract_key_bytes(&self) -> VmrtRes<Vec<u8>> {
        let ec = CastBeKeyFail;
        match self {
            Bool(..) => itr_err_code!(ec),
            _ => self.extract_bytes_with_error_code(ec),
        }
    }

    pub fn check_scalar(&self) -> VmrtErr {
        check_scalar_ec(self, CastBeValueFail)
    }

    pub fn check_tuple_item(&self) -> VmrtErr {
        check_tuple_item_ec(self, CastBeValueFail)
    }

    pub fn extract_call_data(&self, heap: &Heap) -> VmrtRes<Vec<u8>> {
        let ec = CastBeCallDataFail;
        match self {
            Nil => Ok(vec![]),
            HeapSlice((start, length)) => {
                let Value::Bytes(buf) = heap.do_read(*start as usize, *length as usize)? else {
                    never!()
                };
                Ok(buf)
            }
            _ => self.extract_bytes_with_error_code(ec),
        }
    }

    pub fn check_func_argv(&self) -> VmrtErr {
        check_func_boundary_ec(self, CastBeFnArgvFail)?;
        if let Tuple(tuple) = self {
            if tuple.len() > crate::MAX_FUNC_PARAM_LEN {
                return itr_err_fmt!(
                    CastBeFnArgvFail,
                    "func argv length cannot more than {}",
                    crate::MAX_FUNC_PARAM_LEN
                );
            }
        }
        Ok(())
    }

    pub fn check_func_retv(&self) -> VmrtErr {
        check_func_boundary_ec(self, CastBeFnRetvFail)
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

        assert_eq!(hs.extract_call_data(&heap).unwrap(), vec![2, 3]);
        assert!(hs.extract_bytes_with_error_code(CastBeBytesFail).is_err());
        assert!(hs.check_func_argv().is_err());
        assert!(hs.check_func_retv().is_err());

        let tuple = Value::Tuple(
            TupleItem::new(vec![Value::U8(1), Value::Compo(CompoItem::new_list())]).unwrap(),
        );
        assert!(tuple.check_func_argv().is_ok());
        assert!(tuple.check_func_retv().is_ok());
    }

    #[test]
    fn heapslice_func_retv_uses_retv_error_code() {
        let hs = Value::HeapSlice((0, 1));
        let err = hs.check_func_retv().unwrap_err();
        assert_eq!(err.0, ItrErrCode::CastBeFnRetvFail);
    }

    #[test]
    fn func_argv_rejects_tuple_longer_than_param_limit() {
        let tuple = Value::Tuple(
            TupleItem::new(
                (0..(crate::MAX_FUNC_PARAM_LEN + 1))
                    .map(|_| Value::U8(1))
                    .collect(),
            )
            .unwrap(),
        );
        let err = tuple.check_func_argv().unwrap_err();
        assert_eq!(err.0, ItrErrCode::CastBeFnArgvFail);
    }

    #[test]
    fn scalar_check_rejects_compo_and_tuple_values() {
        assert!(Value::Compo(CompoItem::new_list()).check_scalar().is_err());
        assert!(
            Value::Tuple(TupleItem::new(vec![Value::U8(1)]).unwrap())
                .check_scalar()
                .is_err()
        );
    }
}
