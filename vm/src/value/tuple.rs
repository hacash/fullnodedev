#[derive(Default, Debug, Clone, PartialEq, Eq)]
pub struct TupleItem {
    items: Rc<[Value]>,
}

impl TupleItem {
    fn read(&self) -> ReadList<'_> {
        ReadList::Slice(self.as_slice())
    }

    fn check_len(len: usize, max_len: usize) -> VmrtErr {
        if len == 0 {
            return itr_err_code!(CompoPackError);
        }
        if len > max_len {
            return itr_err_code!(OutOfCompoLen);
        }
        Ok(())
    }

    pub fn new(items: Vec<Value>) -> VmrtRes<Self> {
        if items.is_empty() {
            return itr_err_code!(CompoPackError);
        }
        for item in &items {
            item.check_tuple_item()?;
        }
        Ok(Self {
            items: Rc::from(items.into_boxed_slice()),
        })
    }

    pub fn pack(cap: &SpaceCap, ops: &mut Stack) -> VmrtRes<(Value, usize)> {
        let n = ops.pop()?.extract_u16()? as usize;
        Self::check_len(n, cap.tuple_length)?;
        let items = ops.taken(n)?;
        Ok((Value::Tuple(Self::new(items)?), n))
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

    pub fn length(&self, cap: &SpaceCap) -> VmrtRes<Value> {
        self.read().length(cap)
    }

    pub fn haskey(&self, k: Value) -> VmrtRes<Value> {
        self.read().haskey(k)
    }

    pub fn itemget(&self, k: Value) -> VmrtRes<Value> {
        self.read().itemget(k)
    }

    pub fn to_list_with_stats(&self) -> VmrtRes<(Value, usize, usize)> {
        let len = self.items.len();
        let mut bsz = 0usize;
        let mut items = std::collections::VecDeque::with_capacity(len);
        for item in self.items.iter().cloned() {
            bsz += item.val_size();
            items.push_back(item);
        }
        Ok((Value::Compo(CompoItem::list(items)?), len, bsz))
    }

    pub fn to_string(&self) -> String {
        let items: Vec<_> = self.items.iter().map(Value::to_string).collect();
        format!("tuple({})[{}]", self.items.len(), items.join(","))
    }

    pub fn to_json(&self) -> String {
        let items: Vec<_> = self.items.iter().map(Value::to_json).collect();
        format!("{{\"$tuple\":[{}]}}", items.join(","))
    }

    pub fn to_debug_json(&self) -> String {
        let items: Vec<_> = self.items.iter().map(Value::to_debug_json).collect();
        format!("{{\"$tuple\":[{}]}}", items.join(","))
    }

    #[cfg(test)]
    pub(crate) fn shared_count(&self) -> usize {
        Rc::strong_count(&self.items)
    }
}

#[cfg(test)]
mod tuple_tests {
    use super::*;

    #[test]
    fn tuple_rejects_heapslice_and_nested_tuple() {
        let err = TupleItem::new(vec![Value::HeapSlice((0, 1))]).unwrap_err();
        assert_eq!(err.0, ItrErrCode::CastBeValueFail);

        let nested = Value::Tuple(TupleItem::new(vec![Value::U8(1)]).unwrap());
        let err = TupleItem::new(vec![nested]).unwrap_err();
        assert_eq!(err.0, ItrErrCode::CastBeValueFail);
    }

    #[test]
    fn tuple_accepts_compo_items_built_under_scalar_invariant() {
        let tuple = TupleItem::new(vec![Value::Compo(
            CompoItem::list(std::collections::VecDeque::from([Value::U8(1)])).unwrap(),
        )])
        .unwrap();
        assert_eq!(tuple.len(), 1);
    }

    #[test]
    fn tuple_to_list_rejects_compo_item_under_non_nested_compo_rule() {
        let tuple = TupleItem::new(vec![Value::U8(1), Value::Compo(CompoItem::new_map())]).unwrap();
        let err = tuple.to_list_with_stats().unwrap_err();
        assert_eq!(err.0, ItrErrCode::CastBeValueFail);
    }

    #[test]
    fn tuple_to_list_copies_plain_values_and_reports_stats() {
        let tuple = TupleItem::new(vec![Value::U8(7), Value::Bytes(vec![1, 2, 3])]).unwrap();
        let (out, len, bsz) = tuple.to_list_with_stats().unwrap();
        assert_eq!(len, 2);
        assert_eq!(
            bsz,
            Value::U8(7).val_size() + Value::Bytes(vec![1, 2, 3]).val_size()
        );
        let Value::Compo(list) = out else {
            panic!("must be list")
        };
        assert!(list.is_list());
        assert_eq!(list.list_ref().unwrap().len(), 2);
    }

    #[test]
    fn tuple_clone_shares_storage_like_compo_clone() {
        let tuple = TupleItem::new(vec![Value::U8(7), Value::Bytes(vec![1, 2, 3])]).unwrap();
        assert_eq!(tuple.shared_count(), 1);
        let cloned = tuple.clone();
        assert_eq!(tuple.shared_count(), 2);
        assert_eq!(cloned.shared_count(), 2);
    }

    #[test]
    fn tuple_list_reads_follow_list_semantics() {
        let tuple = TupleItem::new(vec![Value::U8(7), Value::Compo(CompoItem::new_map())]).unwrap();
        assert_eq!(tuple.length(&SpaceCap::new(2)).unwrap(), Value::U32(2));
        assert_eq!(tuple.haskey(Value::U8(1)).unwrap(), Value::Bool(true));
        assert_eq!(tuple.haskey(Value::U8(2)).unwrap(), Value::Bool(false));
        assert!(matches!(
            tuple.itemget(Value::U8(1)).unwrap(),
            Value::Compo(_)
        ));
    }

    #[test]
    fn tuple_length_checks_compo_cap() {
        let tuple = TupleItem::new(vec![Value::U8(1), Value::U8(2)]).unwrap();
        let mut cap = SpaceCap::new(1);
        cap.compo_length = 1;
        let err = tuple.length(&cap).unwrap_err();
        assert_eq!(err.0, ItrErrCode::OutOfCompoLen);
    }

    #[test]
    fn tuple_pack_uses_tuple_length_cap() {
        let mut cap = SpaceCap::new(1);
        cap.tuple_length = 1;
        let mut ops = Stack::new(8);
        ops.push(Value::U8(1)).unwrap();
        ops.push(Value::U8(2)).unwrap();
        ops.push(Value::U8(2)).unwrap();
        let err = TupleItem::pack(&cap, &mut ops).unwrap_err();
        assert_eq!(err.0, ItrErrCode::OutOfCompoLen);
    }

    #[test]
    fn tuple_pack_is_decoupled_from_compo_length() {
        let mut cap = SpaceCap::new(1);
        cap.tuple_length = 2;
        cap.compo_length = 1;
        let mut ops = Stack::new(8);
        ops.push(Value::U8(1)).unwrap();
        ops.push(Value::U8(2)).unwrap();
        ops.push(Value::U8(2)).unwrap();
        let (value, _) = TupleItem::pack(&cap, &mut ops).unwrap();
        let Value::Tuple(tuple) = value else {
            panic!("must be tuple")
        };
        assert_eq!(tuple.len(), 2);
    }
}
