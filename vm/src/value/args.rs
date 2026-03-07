
#[derive(Default, Debug, Clone, PartialEq, Eq)]
pub struct ArgsItem {
    items: Box<[Value]>,
}

impl ArgsItem {
    pub fn new(items: Vec<Value>) -> VmrtRes<Self> {
        if items.is_empty() {
            return itr_err_code!(CompoPackError)
        }
        for item in &items {
            Value::check_func_argv_item(item, CastBeFnArgvFail)?;
        }
        Ok(Self {
            items: items.into_boxed_slice(),
        })
    }

    pub fn pack(cap: &SpaceCap, ops: &mut Stack) -> VmrtRes<(Value, usize)> {
        let n = ops.pop()?.checked_u16()? as usize;
        if n == 0 {
            return itr_err_code!(CompoPackError)
        }
        if n > cap.max_compo_length {
            return itr_err_code!(OutOfCompoLen)
        }
        let items = ops.taken(n)?;
        Ok((Value::Args(Self::new(items)?), n))
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    pub fn as_slice(&self) -> &[Value] {
        &self.items
    }

    pub fn to_vec(&self) -> Vec<Value> {
        self.items.to_vec()
    }

    pub fn val_size(&self) -> usize {
        self.items.iter().map(Value::val_size).sum()
    }

    pub fn to_string(&self) -> String {
        let items: Vec<_> = self.items.iter().map(Value::to_string).collect();
        format!("args({})[{}]", self.items.len(), items.join(","))
    }

    pub fn to_json(&self) -> String {
        let items: Vec<_> = self.items.iter().map(Value::to_json).collect();
        format!("{{\"$args\":[{}]}}", items.join(","))
    }

    pub fn to_debug_json(&self) -> String {
        let items: Vec<_> = self.items.iter().map(Value::to_debug_json).collect();
        format!("{{\"$args\":[{}]}}", items.join(","))
    }
}

#[cfg(test)]
mod args_tests {
    use super::*;

    #[test]
    fn args_reject_heapslice_and_nested_args() {
        let err = ArgsItem::new(vec![Value::HeapSlice((0, 1))]).unwrap_err();
        assert_eq!(err.0, ItrErrCode::CastBeFnArgvFail);

        let nested = Value::Args(ArgsItem::new(vec![Value::U8(1)]).unwrap());
        let err = ArgsItem::new(vec![nested]).unwrap_err();
        assert_eq!(err.0, ItrErrCode::CastBeFnArgvFail);
    }
}
