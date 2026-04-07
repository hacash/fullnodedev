fn is_scalar_value(value: &Value) -> bool {
    matches!(
        value,
        Nil | Bool(..) | U8(..) | U16(..) | U32(..) | U64(..) | U128(..) | Bytes(..) | Address(..)
    )
}

fn check_scalar_as(value: &Value, ec: ItrErrCode) -> VmrtErr {
    if is_scalar_value(value) {
        Ok(())
    } else {
        itr_err_code!(ec)
    }
}

fn check_func_tuple_item(value: &Value, ec: ItrErrCode) -> VmrtErr {
    match value {
        HeapSlice(..) | Tuple(..) => itr_err_code!(ec),
        Compo(..) | Handle(..) => Ok(()),
        _ => check_scalar_as(value, ec),
    }
}

fn check_func_boundary(value: &Value, ec: ItrErrCode) -> VmrtErr {
    match value {
        HeapSlice(..) => itr_err_code!(ec),
        Tuple(tuple) => {
            for item in tuple.as_slice() {
                check_func_tuple_item(item, ec)?;
            }
            Ok(())
        }
        Compo(..) | Handle(..) => Ok(()),
        _ => check_scalar_as(value, ec),
    }
}

fn check_vm_boundary_compo(compo: &CompoItem, ec: ItrErrCode) -> VmrtErr {
    if let Ok(list) = compo.list_ref() {
        for item in list {
            check_scalar_as(item, ec)?;
        }
        return Ok(());
    }
    for value in compo.map_ref()?.values() {
        check_scalar_as(value, ec)?;
    }
    Ok(())
}

fn check_vm_tuple_item(value: &Value, ec: ItrErrCode) -> VmrtErr {
    match value {
        HeapSlice(..) | Tuple(..) | Handle(..) => itr_err_code!(ec),
        Compo(compo) => check_vm_boundary_compo(compo, ec),
        _ => check_scalar_as(value, ec),
    }
}

fn check_vm_boundary(value: &Value, ec: ItrErrCode) -> VmrtErr {
    match value {
        HeapSlice(..) => itr_err_code!(ec),
        Tuple(tuple) => {
            for item in tuple.as_slice() {
                check_vm_tuple_item(item, ec)?;
            }
            Ok(())
        }
        Compo(compo) => check_vm_boundary_compo(compo, ec),
        Handle(..) => itr_err_code!(ec),
        _ => check_scalar_as(value, ec),
    }
}

impl Value {
    pub fn check_non_nil_scalar(&self, nil_ec: ItrErrCode) -> VmrtErr {
        if matches!(self, Nil) {
            return itr_err_code!(nil_ec);
        }
        check_scalar_as(self, CastBeValueFail)
    }

    pub fn check_boundary_value_cap(&self, cap: &SpaceCap) -> VmrtErr {
        match self {
            Tuple(tuple) => {
                for item in tuple.as_slice() {
                    item.check_boundary_value_cap(cap)?;
                }
                Ok(())
            }
            Compo(compo) => {
                if let Ok(list) = compo.list_ref() {
                    for item in list {
                        item.check_boundary_value_cap(cap)?;
                    }
                    return Ok(());
                }
                for (key, value) in compo.map_ref()? {
                    if key.len() > cap.value_size {
                        return itr_err_code!(OutOfValueSize);
                    }
                    value.check_boundary_value_cap(cap)?;
                }
                Ok(())
            }
            _ => {
                self.clone().valid(cap)?;
                Ok(())
            }
        }
    }

    pub(crate) fn extract_bytes_len_with_error_code(&self, ec: ItrErrCode) -> VmrtRes<usize> {
        match self {
            Bool(..) | U8(..) => Ok(1),
            U16(..) => Ok(2),
            U32(..) => Ok(4),
            U64(..) => Ok(8),
            U128(..) => Ok(16),
            Bytes(b) => Ok(b.len()),
            Address(..) => Ok(field::Address::SIZE),
            _ => itr_err_code!(ec),
        }
    }

    fn extract_bytes_with_error_code(&self, ec: ItrErrCode) -> VmrtRes<Vec<u8>> {
        match self.scalar_bytes() {
            Some(bytes) => Ok(bytes),
            None => itr_err_code!(ec),
        }
    }

    pub fn extract_bytes(&self) -> VmrtRes<Vec<u8>> {
        self.extract_bytes_with_error_code(CastBeBytesFail)
    }

    pub(crate) fn extract_key_bytes_with_error_code(&self, ec: ItrErrCode) -> VmrtRes<Vec<u8>> {
        let key = match self {
            Bool(..) => return itr_err_code!(ec),
            _ => self.extract_bytes_with_error_code(ec)?,
        };
        if key.is_empty() {
            return itr_err_code!(ec);
        }
        Ok(key)
    }

    pub fn extract_key_bytes(&self) -> VmrtRes<Vec<u8>> {
        self.extract_key_bytes_with_error_code(CastBeKeyFail)
    }

    pub fn check_scalar(&self) -> VmrtErr {
        check_scalar_as(self, CastBeValueFail)
    }

    pub fn check_tuple_item(&self) -> VmrtErr {
        check_func_tuple_item(self, CastBeValueFail)
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
        check_func_boundary(self, CastBeFnArgvFail)?;
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
        check_func_boundary(self, CastBeFnRetvFail)
    }

    pub fn check_vm_boundary_argv(&self) -> VmrtErr {
        check_vm_boundary(self, CastBeFnArgvFail)?;
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

    pub fn check_vm_boundary_retv(&self) -> VmrtErr {
        match self {
            Value::HeapSlice(..) => {
                itr_err_fmt!(CastBeFnRetvFail, "return type HeapSlice is not supported")
            }
            Value::Handle(..) => {
                itr_err_fmt!(CastBeFnRetvFail, "return type Handle is not supported")
            }
            _ => check_vm_boundary(self, CastBeFnRetvFail),
        }
    }

    pub fn check_container_cap(&self, cap: &SpaceCap) -> VmrtErr {
        match self {
            Tuple(tuple) => {
                if tuple.len() > cap.tuple_length {
                    return itr_err_code!(OutOfCompoLen);
                }
                for item in tuple.as_slice() {
                    item.check_container_cap(cap)?;
                }
                Ok(())
            }
            Compo(compo) => {
                if compo.len() > cap.compo_length {
                    return itr_err_code!(OutOfCompoLen);
                }
                Ok(())
            }
            _ => Ok(()),
        }
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

        let handle = Value::handle(7u32);
        assert!(handle.check_func_argv().is_ok());
        assert!(handle.check_func_retv().is_ok());
        assert!(handle.check_vm_boundary_argv().is_err());
        assert!(handle.check_vm_boundary_retv().is_err());
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
    fn vm_boundary_rejects_tuple_with_handle_item() {
        let tuple = Value::Tuple(
            TupleItem::new(vec![Value::U8(1), Value::handle(7u32)]).unwrap(),
        );
        let err = tuple.check_vm_boundary_retv().unwrap_err();
        assert_eq!(err.0, ItrErrCode::CastBeFnRetvFail);
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

    #[test]
    fn extract_key_bytes_rejects_empty_and_bool_keys() {
        let nil_err = Value::Nil.extract_key_bytes().unwrap_err();
        assert_eq!(nil_err.0, ItrErrCode::CastBeKeyFail);

        let empty_err = Value::Bytes(vec![]).extract_key_bytes().unwrap_err();
        assert_eq!(empty_err.0, ItrErrCode::CastBeKeyFail);

        let bool_err = Value::Bool(true).extract_key_bytes().unwrap_err();
        assert_eq!(bool_err.0, ItrErrCode::CastBeKeyFail);

        assert_eq!(Value::Bytes(vec![1]).extract_key_bytes().unwrap(), vec![1]);
    }

    #[test]
    fn container_cap_rejects_oversize_compo_nested_in_tuple() {
        let tuple = Value::Tuple(
            TupleItem::new(vec![Value::Compo(
                CompoItem::list(std::collections::VecDeque::from([Value::U8(1), Value::U8(2)]))
                    .unwrap(),
            )])
            .unwrap(),
        );
        let mut cap = SpaceCap::new(1);
        cap.compo_length = 1;
        let err = tuple.check_container_cap(&cap).unwrap_err();
        assert_eq!(err.0, ItrErrCode::OutOfCompoLen);
    }

    #[test]
    fn boundary_value_cap_rejects_oversize_scalar_and_map_key() {
        let mut cap = SpaceCap::new(1);
        cap.value_size = 2;

        let err = Value::Bytes(vec![0u8; 3]).check_boundary_value_cap(&cap).unwrap_err();
        assert_eq!(err.0, ItrErrCode::OutOfValueSize);

        let mut map = std::collections::BTreeMap::new();
        map.insert(vec![0u8; 3], Value::U8(1));
        let err = Value::Compo(CompoItem::map(map).unwrap())
            .check_boundary_value_cap(&cap)
            .unwrap_err();
        assert_eq!(err.0, ItrErrCode::OutOfValueSize);
    }
}
