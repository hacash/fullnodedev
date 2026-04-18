#[derive(Debug, Clone)]
enum Compo {
    List(VecDeque<Value>),
    Map(BTreeMap<Vec<u8>, Value>),
}

impl PartialEq for Compo {
    fn eq(&self, _: &Self) -> bool {
        // Intentionally not VM semantic equality.
        // `Compo` lives behind `CompoItem(Rc<UnsafeCell<_>>)` and Rust `==` must not be treated
        // as contract-visible value comparison. VM content equality is implemented separately via
        // `value_content_eq` / `CompoItem::content_eq`.
        false
    }
}

impl Eq for Compo {}

impl Default for Compo {
    fn default() -> Self {
        Self::List(VecDeque::new())
    }
}

macro_rules! ret_invalid_compo_op {
    () => {
        return itr_err_code!(CompoOpInvalid)
    };
}

macro_rules! checked_compo_op_len {
    ($i:expr, $a: expr) => {
        if $i as usize > $a.len() {
            return itr_err_code!(CompoOpOverflow);
        }
    };
}

impl Compo {
    fn to_string(&self) -> String {
        match self {
            Self::List(a) => {
                let sss: Vec<_> = a.iter().map(|a| a.to_string()).collect();
                format!("[{}]", sss.join(","))
            }
            Self::Map(b) => {
                let mmm: Vec<_> = b
                    .iter()
                    .map(|(k, v)| format!("0x{}:{}", k.to_hex(), v.to_string()))
                    .collect();
                format!("{{{}}}", mmm.join(","))
            }
        }
    }

    fn to_json(&self) -> String {
        match self {
            Self::List(a) => {
                let sss: Vec<_> = a.iter().map(|a| a.to_json()).collect();
                format!("[{}]", sss.join(","))
            }
            Self::Map(b) => match Self::map_debug_json(b) {
                Some(s) => s,
                None => {
                    let mmm: Vec<_> = b
                        .iter()
                        .map(|(k, v)| format!(r#"{{"key_hex":"{}","value":{}}}"#, k.to_hex(), v.to_json()))
                        .collect();
                    format!(r#"{{"$map":[{}]}}"#, mmm.join(","))
                }
            }
        }
    }

    fn to_debug_json(&self) -> String {
        match self {
            Self::List(a) => {
                let sss: Vec<_> = a.iter().map(Value::to_debug_json).collect();
                format!("[{}]", sss.join(","))
            }
            Self::Map(b) => match Self::map_debug_json(b) {
                Some(s) => s,
                None => {
                    let mmm: Vec<_> = b
                        .iter()
                        .map(|(k, v)| match bytes_try_to_readable_string(k) {
                            Some(s) => format!(
                                r#"{{"key":{},"key_hex":"{}","value":{}}}"#,
                                serde_json::to_string(&s).unwrap(),
                                k.to_hex(),
                                v.to_debug_json()
                            ),
                            None => format!(
                                r#"{{"key_hex":"{}","value":{}}}"#,
                                k.to_hex(),
                                v.to_debug_json()
                            ),
                        })
                        .collect();
                    format!(r#"{{"$map":[{}]}}"#, mmm.join(","))
                }
            },
        }
    }

    fn map_debug_json(items: &BTreeMap<Vec<u8>, Value>) -> Option<String> {
        let mut mmm = Vec::with_capacity(items.len());
        for (k, v) in items {
            let key = bytes_try_to_readable_string(k)?;
            mmm.push(format!(
                "{}:{}",
                serde_json::to_string(&key).unwrap(),
                v.to_debug_json()
            ));
        }
        Some(format!("{{{}}}", mmm.join(",")))
    }

    fn len(&self) -> usize {
        match self {
            Self::List(a) => a.len(),
            Self::Map(b) => b.len(),
        }
    }

    fn val_size(&self) -> usize {
        match self {
            Self::List(items) => {
                let mut sum = 0usize;
                for v in items {
                    sum = add_size_saturating(sum, v.val_size());
                    if sum == usize::MAX {
                        break;
                    }
                }
                sum
            }
            Self::Map(items) => {
                let mut sum = 0usize;
                for (k, v) in items {
                    sum = add_size_saturating(sum, k.len());
                    if sum == usize::MAX {
                        break;
                    }
                    sum = add_size_saturating(sum, v.val_size());
                    if sum == usize::MAX {
                        break;
                    }
                }
                sum
            }
        }
    }

    pub fn clear(&mut self) {
        match self {
            Self::List(a) => a.clear(),
            Self::Map(b) => b.clear(),
        }
    }

    fn append(&mut self, cap: &SpaceCap, v: Value) -> VmrtErr {
        v.check_scalar()?;
        match self {
            Self::List(a) => {
                if a.len() >= cap.compo_length {
                    return itr_err_code!(OutOfCompoLen);
                }
                a.push_back(v)
            }
            _ => ret_invalid_compo_op! {},
        }
        Ok(())
    }

    fn remove(&mut self, k: Value) -> VmrtErr {
        match self {
            Self::List(a) => {
                let i = k.extract_u32()?;
                if i as usize >= a.len() {
                    return itr_err_code!(CompoNoFindItem);
                }
                a.remove(i as usize);
            }
            Self::Map(b) => {
                let k = k.extract_key_bytes()?;
                if b.remove(&k).is_none() {
                    return itr_err_code!(CompoNoFindItem);
                }
            }
        }
        Ok(())
    }

    fn insert(&mut self, cap: &SpaceCap, k: Value, v: Value) -> VmrtErr {
        v.check_scalar()?;
        match self {
            Self::List(a) => {
                let i = k.extract_u32()?;
                checked_compo_op_len! {i, a};
                if a.len() >= cap.compo_length {
                    return itr_err_code!(OutOfCompoLen);
                }
                a.insert(i as usize, v);
            }
            Self::Map(b) => {
                let k = k.extract_key_bytes()?;
                if !b.contains_key(&k) && b.len() >= cap.compo_length {
                    return itr_err_code!(OutOfCompoLen);
                }
                b.insert(k, v);
            }
        }
        Ok(())
    }

    // return Bool
    fn haskey(&self, k: Value) -> VmrtRes<Value> {
        match self {
            Self::List(a) => ReadList::Deque(a).haskey(k),
            Self::Map(b) => {
                let k = k.extract_key_bytes()?;
                Ok(Value::Bool(b.contains_key(&k)))
            }
        }
    }

    fn itemget(&self, k: Value) -> VmrtRes<Value> {
        let v = match self {
            Self::List(a) => return ReadList::Deque(a).itemget(k),
            Self::Map(b) => {
                let nfer = || itr_err_code!(CompoNoFindItem);
                let k = k.extract_key_bytes()?;
                match b.get(&k) {
                    Some(b) => b.clone(),
                    _ => return nfer(), // error not find
                }
            }
        };
        Ok(v)
    }
}

/**********************************************************/

#[derive(Default, Clone)]
pub struct CompoItem {
    compo: Rc<UnsafeCell<Compo>>,
}

impl Display for CompoItem {
    fn fmt(&self, f: &mut Formatter) -> Result {
        write!(f, "{}", self.to_json())
    }
}

impl Debug for CompoItem {
    fn fmt(&self, f: &mut Formatter) -> Result {
        write!(f, "{}", self.to_string())
    }
}

impl PartialEq for CompoItem {
    fn eq(&self, other: &Self) -> bool {
        // Intentionally pointer identity, not VM semantic equality.
        // This supports runtime/ref semantics and cheap identity checks. Any contract-visible
        // comparison must use `value_content_eq` / `CompoItem::content_eq` instead.
        self.ptr_eq(other)
    }
}

impl Eq for CompoItem {}

macro_rules! get_compo_inner_ref {
    ($self: ident) => {
        unsafe { &*$self.compo.get() }
    };
}

macro_rules! get_compo_inner_mut {
    ($self: ident) => {
        unsafe { &mut *$self.compo.get() }
    };
}

macro_rules! get_compo_inner_by {
    ($self: ident, $ty: ident, $inner: ident) => {{
        let r = $inner!($self);
        let Compo::$ty(obj) = r else {
            return itr_err_code!(CompoOpNotMatch);
        };
        Ok(obj)
    }};
}

macro_rules! take_items_from_ops {
    ($is_map: expr, $cap: expr, $ops: expr) => {{
        let n = $ops.pop()?.extract_u16()? as usize;
        if n == 0 {
            return itr_err_code!(CompoPackError);
        }
        let mut max = $cap.compo_length;
        if $is_map {
            max *= 2; // for k => v
        }
        if n > max {
            return itr_err_code!(OutOfCompoLen);
        }
        let items = $ops.taken(n)?;
        items
    }};
}

impl CompoItem {
    pub fn to_string(&self) -> String {
        get_compo_inner_ref!(self).to_string()
    }

    pub fn to_json(&self) -> String {
        get_compo_inner_ref!(self).to_json()
    }

    pub fn to_debug_json(&self) -> String {
        get_compo_inner_ref!(self).to_debug_json()
    }
}

impl CompoItem {
    pub fn list(l: VecDeque<Value>) -> VmrtRes<Self> {
        for item in &l {
            item.check_scalar()?;
        }
        Ok(Self {
            compo: Rc::new(UnsafeCell::new(Compo::List(l))),
        })
    }

    pub fn map(m: BTreeMap<Vec<u8>, Value>) -> VmrtRes<Self> {
        for v in m.values() {
            v.check_scalar()?;
        }
        Ok(Self {
            compo: Rc::new(UnsafeCell::new(Compo::Map(m))),
        })
    }

    pub fn pack_list(cap: &SpaceCap, ops: &mut Stack) -> VmrtRes<(Value, usize)> {
        let items = take_items_from_ops!(false, cap, ops);
        let len = items.len();
        for item in &items {
            item.check_scalar()?;
        }
        Ok((Value::Compo(Self::list(VecDeque::from(items))?), len))
    }

    pub fn pack_map(cap: &SpaceCap, ops: &mut Stack) -> VmrtRes<(Value, usize)> {
        let mut items: Vec<_> = take_items_from_ops!(true, cap, ops)
            .into_iter()
            .map(|a| Some(a))
            .collect();
        let m = items.len();
        if m % 2 != 0 {
            return itr_err_code!(CompoPackError); // map must k => v
        }
        let pair_count = m / 2;
        let mut mapobj = BTreeMap::new();
        for i in 0..pair_count {
            let k = items[i * 2].take().unwrap();
            let v = items[i * 2 + 1].take().unwrap();
            let k = k.extract_key_bytes()?;
            v.check_scalar()?;
            if mapobj.insert(k, v).is_some() {
                return itr_err_fmt!(CompoPackError, "duplicate key in pack_map");
            }
        }
        Ok((Value::Compo(Self::map(mapobj)?), m))
    }

    pub fn is_list(&self) -> bool {
        match get_compo_inner_ref!(self) {
            Compo::List(..) => true,
            _ => false,
        }
    }

    pub fn is_map(&self) -> bool {
        match get_compo_inner_ref!(self) {
            Compo::Map(..) => true,
            _ => false,
        }
    }

    pub fn list_ref(&self) -> VmrtRes<&VecDeque<Value>> {
        get_compo_inner_by!(self, List, get_compo_inner_ref)
    }

    pub fn map_ref(&self) -> VmrtRes<&BTreeMap<Vec<u8>, Value>> {
        get_compo_inner_by!(self, Map, get_compo_inner_ref)
    }

    fn list_mut(&self) -> VmrtRes<&mut VecDeque<Value>> {
        get_compo_inner_by!(self, List, get_compo_inner_mut)
    }

    #[allow(unused)]
    fn map_mut(&self) -> VmrtRes<&mut BTreeMap<Vec<u8>, Value>> {
        get_compo_inner_by!(self, Map, get_compo_inner_mut)
    }

    pub fn new_list() -> Self {
        Self {
            compo: Rc::new(UnsafeCell::new(Compo::List(VecDeque::new()))),
        }
    }

    pub fn new_map() -> Self {
        Self {
            compo: Rc::new(UnsafeCell::new(Compo::Map(BTreeMap::new()))),
        }
    }

    pub fn copy(&self) -> Self {
        self.copy_with_stats().0
    }

    pub fn copy_with_stats(&self) -> (Self, usize, usize) {
        let (data, len, bsz) = match get_compo_inner_ref!(self) {
            Compo::List(src) => {
                let len = src.len();
                let mut bsz = 0usize;
                let mut list = VecDeque::with_capacity(len);
                for v in src.iter() {
                    bsz = add_size_saturating(bsz, v.val_size());
                    list.push_back(v.clone());
                }
                (Compo::List(list), len, bsz)
            }
            Compo::Map(src) => {
                let len = src.len();
                let mut bsz = 0usize;
                let mut map = BTreeMap::new();
                for (k, v) in src.iter() {
                    bsz = add_size_saturating(bsz, k.len());
                    bsz = add_size_saturating(bsz, v.val_size());
                    map.insert(k.clone(), v.clone());
                }
                (Compo::Map(map), len, bsz)
            }
        };
        (
            Self {
                compo: Rc::new(UnsafeCell::new(data)),
            },
            len,
            bsz,
        )
    }

    pub fn ptr_eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.compo, &other.compo)
    }

    pub fn merge(&mut self, cap: &SpaceCap, compo: CompoItem) -> VmrtErr {
        self.merge_with_stats(cap, compo).map(|_| ())
    }

    pub fn merge_with_stats(
        &mut self,
        cap: &SpaceCap,
        compo: CompoItem,
    ) -> VmrtRes<(usize, usize)> {
        if Rc::ptr_eq(&self.compo, &compo.compo) {
            return itr_err_code!(CompoOpInvalid);
        }
        match get_compo_inner_mut!(self) {
            Compo::List(l) => {
                let src = compo.list_ref()?.clone();
                let src_len = src.len();
                let new_len = l.len() + src_len;
                if new_len > cap.compo_length {
                    return itr_err_code!(OutOfCompoLen);
                }
                let mut src_bsz = 0usize;
                for v in src.iter() {
                    v.check_scalar()?;
                    src_bsz = add_size_saturating(src_bsz, v.val_size());
                }
                l.extend(src);
                Ok((src_len, src_bsz))
            }
            Compo::Map(m) => {
                let src = compo.map_ref()?.clone();
                let src_len = src.len();
                let mut add = 0usize;
                let mut src_bsz = 0usize;
                for (k, v) in src.iter() {
                    v.check_scalar()?;
                    src_bsz = add_size_saturating(src_bsz, k.len());
                    src_bsz = add_size_saturating(src_bsz, v.val_size());
                    if !m.contains_key(k) {
                        add += 1;
                    }
                }
                if m.len() + add > cap.compo_length {
                    return itr_err_code!(OutOfCompoLen);
                }
                m.extend(src);
                Ok((src_len, src_bsz))
            }
        }
    }
}

impl CompoItem {
    pub fn len(&self) -> usize {
        get_compo_inner_ref!(self).len()
    }

    pub fn val_size(&self) -> usize {
        get_compo_inner_ref!(self).val_size()
    }

    pub fn length(&self, cap: &SpaceCap) -> VmrtRes<Value> {
        match get_compo_inner_ref!(self) {
            Compo::List(a) => ReadList::Deque(a).length(cap),
            Compo::Map(b) => length_value_by_len(cap, b.len()),
        }
    }

    pub fn haskey(&self, k: Value) -> VmrtRes<Value> {
        get_compo_inner_ref!(self).haskey(k)
    }

    pub fn remove(&mut self, k: Value) -> VmrtErr {
        let compo = get_compo_inner_mut!(self);
        compo.remove(k)
    }

    pub fn insert(&mut self, cap: &SpaceCap, k: Value, v: Value) -> VmrtErr {
        let compo = get_compo_inner_mut!(self);
        compo.insert(cap, k, v)
    }

    pub fn clear(&mut self) {
        let compo = get_compo_inner_mut!(self);
        compo.clear()
    }

    pub fn append(&mut self, cap: &SpaceCap, v: Value) -> VmrtErr {
        let compo = get_compo_inner_mut!(self);
        compo.append(cap, v)
    }

    pub fn itemget(&self, k: Value) -> VmrtRes<Value> {
        let compo = get_compo_inner_ref!(self);
        compo.itemget(k)
    }

    pub fn keys(&self) -> VmrtRes<Value> {
        let map = self.map_ref()?;
        let keys = map.keys().map(|k| Value::Bytes(k.clone())).collect();
        Ok(Value::Compo(Self::list(keys)?))
    }

    pub fn keys_with_stats(&self) -> VmrtRes<(Value, usize, usize)> {
        let map = self.map_ref()?;
        let mut bsz = 0usize;
        let mut keys = VecDeque::with_capacity(map.len());
        for k in map.keys() {
            bsz = add_size_saturating(bsz, k.len());
            Value::Bytes(k.clone()).can_get_size()?;
            keys.push_back(Value::Bytes(k.clone()));
        }
        Ok((Value::Compo(Self::list(keys)?), map.len(), bsz))
    }

    pub fn values(&self) -> VmrtRes<Value> {
        let map = self.map_ref()?;
        let values = map.values().map(|v| v.clone()).collect();
        Ok(Value::Compo(Self::list(values)?))
    }

    pub fn values_with_stats(&self) -> VmrtRes<(Value, usize, usize)> {
        let map = self.map_ref()?;
        let mut bsz = 0usize;
        let mut values = VecDeque::with_capacity(map.len());
        for v in map.values() {
            bsz = add_size_saturating(bsz, v.val_size());
            values.push_back(v.clone());
        }
        Ok((Value::Compo(Self::list(values)?), map.len(), bsz))
    }

    pub fn content_eq(&self, other: &Self) -> VmrtRes<bool> {
        if self.ptr_eq(other) {
            return Ok(true);
        }
        match (self.list_ref(), other.list_ref()) {
            (Ok(lhs), Ok(rhs)) => {
                if lhs.len() != rhs.len() {
                    return Ok(false);
                }
                for (l, r) in lhs.iter().zip(rhs.iter()) {
                    if !value_content_eq(l, r)? {
                        return Ok(false);
                    }
                }
                return Ok(true);
            }
            (Err(_), Err(_)) => {}
            _ => return Ok(false),
        }

        let lhs = self.map_ref()?;
        let rhs = other.map_ref()?;
        if lhs.len() != rhs.len() {
            return Ok(false);
        }
        for (key, lval) in lhs.iter() {
            let Some(rval) = rhs.get(key) else {
                return Ok(false);
            };
            if !value_content_eq(lval, rval)? {
                return Ok(false);
            }
        }
        Ok(true)
    }

    pub fn compare_fee(&self, other: &Self, container_header_fee: usize) -> usize {
        if self.ptr_eq(other) {
            return container_header_fee;
        }
        match (self.list_ref(), other.list_ref()) {
            (Ok(lhs), Ok(rhs)) => {
                if lhs.len() != rhs.len() {
                    return container_header_fee;
                }
                let mut fee = container_header_fee;
                for (l, r) in lhs.iter().zip(rhs.iter()) {
                    fee = add_size_saturating(fee, value_compare_fee(l, r, container_header_fee));
                    if fee == usize::MAX {
                        break;
                    }
                }
                return fee;
            }
            (Err(_), Err(_)) => {}
            _ => return container_header_fee,
        }

        let Ok(lhs) = self.map_ref() else {
            return container_header_fee;
        };
        let Ok(rhs) = other.map_ref() else {
            return container_header_fee;
        };
        if lhs.len() != rhs.len() {
            return container_header_fee;
        }
        let mut fee = container_header_fee;
        for (key, lval) in lhs.iter() {
            fee = add_size_saturating(fee, key.len());
            if fee == usize::MAX {
                break;
            }
            let Some(rval) = rhs.get(key) else {
                break;
            };
            fee = add_size_saturating(fee, value_compare_fee(lval, rval, container_header_fee));
            if fee == usize::MAX {
                break;
            }
        }
        fee
    }

    pub fn head(&mut self) -> VmrtRes<Value> {
        let list = self.list_mut()?;
        match list.pop_front() {
            Some(v) => Ok(v),
            _ => itr_err_code!(CompoOpOverflow),
        }
    }

    /// Returns the last element of the list.
    /// e.g. back([10, 20, 30]) -> 30
    pub fn back(&mut self) -> VmrtRes<Value> {
        let list = self.list_mut()?;
        match list.pop_back() {
            Some(v) => Ok(v),
            _ => itr_err_code!(CompoOpOverflow),
        }
    }
}

#[cfg(test)]
mod compo_tests {
    use super::*;

    #[test]
    fn compo_rejects_tuple_and_compo_children() {
        let tuple = Value::Tuple(TupleItem::new(vec![Value::U8(1)]).unwrap());
        let err = CompoItem::list(VecDeque::from([tuple])).unwrap_err();
        assert_eq!(err.0, ItrErrCode::CastBeValueFail);

        let err =
            CompoItem::list(VecDeque::from([Value::Compo(CompoItem::new_map())])).unwrap_err();
        assert_eq!(err.0, ItrErrCode::CastBeValueFail);

        let err =
            CompoItem::list(VecDeque::from([Value::handle(7u32)])).unwrap_err();
        assert_eq!(err.0, ItrErrCode::CastBeValueFail);
    }

    #[test]
    fn compo_append_rejects_tuple_values() {
        let mut compo = CompoItem::new_list();
        let err = compo
            .append(
                &SpaceCap::new(1),
                Value::Tuple(TupleItem::new(vec![Value::U8(1)]).unwrap()),
        )
        .unwrap_err();
        assert_eq!(err.0, ItrErrCode::CastBeValueFail);
    }

    #[test]
    fn compo_append_overflow_keeps_list_unchanged() {
        let mut cap = SpaceCap::new(1);
        cap.compo_length = 1;
        let mut compo = CompoItem::new_list();
        compo.append(&cap, Value::U8(1)).unwrap();
        let err = compo.append(&cap, Value::U8(2)).unwrap_err();
        assert_eq!(err.0, ItrErrCode::OutOfCompoLen);
        assert_eq!(
            compo.list_ref().unwrap().iter().cloned().collect::<Vec<_>>(),
            vec![Value::U8(1)]
        );
    }

    #[test]
    fn compo_insert_rejects_tuple_values() {
        let mut compo = CompoItem::new_map();
        let err = compo
            .insert(
                &SpaceCap::new(1),
                Value::Bytes(vec![1]),
                Value::Tuple(TupleItem::new(vec![Value::U8(1)]).unwrap()),
        )
        .unwrap_err();
        assert_eq!(err.0, ItrErrCode::CastBeValueFail);

        let err = compo
            .insert(
                &SpaceCap::new(1),
                Value::Bytes(vec![2]),
                Value::handle(9u32),
        )
        .unwrap_err();
        assert_eq!(err.0, ItrErrCode::CastBeValueFail);
    }

    #[test]
    fn compo_insert_overflow_keeps_map_unchanged() {
        let mut cap = SpaceCap::new(1);
        cap.compo_length = 1;
        let mut compo = CompoItem::new_map();
        let k1 = Value::Bytes(vec![1]);
        let k2 = Value::Bytes(vec![2]);
        compo.insert(&cap, k1.clone(), Value::U8(1)).unwrap();
        compo.insert(&cap, k1.clone(), Value::U8(9)).unwrap();
        let err = compo.insert(&cap, k2.clone(), Value::U8(2)).unwrap_err();
        assert_eq!(err.0, ItrErrCode::OutOfCompoLen);
        let map = compo.map_ref().unwrap();
        assert_eq!(map.len(), 1);
        assert_eq!(map.get(&vec![1]).unwrap(), &Value::U8(9));
        assert!(!map.contains_key(&vec![2]));
    }

    #[test]
    fn merged_compo_remains_valid_as_single_function_argument() {
        let cap = SpaceCap::new(1);
        let mut dst = CompoItem::new_list();
        dst.append(&cap, Value::U8(1)).unwrap();
        let src = CompoItem::list(VecDeque::from([Value::U16(2)])).unwrap();
        dst.merge(&cap, src).unwrap();
        assert!(Value::Compo(dst).check_func_argv().is_ok());
    }

    #[test]
    fn compo_equality_uses_shared_identity() {
        let compo = CompoItem::list(VecDeque::from([Value::U8(1)])).unwrap();
        let shared = compo.clone();
        let rebuilt = CompoItem::list(VecDeque::from([Value::U8(1)])).unwrap();

        assert_eq!(compo, shared);
        assert_ne!(compo, rebuilt);
        assert_eq!(Value::Compo(compo), Value::Compo(shared));
        assert_ne!(Value::Compo(rebuilt.clone()), Value::Compo(CompoItem::copy(&rebuilt)));
    }

    #[test]
    fn compo_content_eq_uses_value_semantics_with_ptr_fast_path() {
        let shared = CompoItem::list(VecDeque::from([Value::U8(1), Value::Bytes(vec![2, 3])])).unwrap();
        let cloned = shared.clone();
        let rebuilt = CompoItem::list(VecDeque::from([Value::U8(1), Value::Bytes(vec![2, 3])])).unwrap();
        let diff = CompoItem::list(VecDeque::from([Value::U8(1), Value::Bytes(vec![2, 4])])).unwrap();

        assert!(shared.content_eq(&cloned).unwrap());
        assert!(shared.content_eq(&rebuilt).unwrap());
        assert!(!shared.content_eq(&diff).unwrap());
        assert_eq!(shared.compare_fee(&cloned, 16), 16);
        assert!(shared.compare_fee(&rebuilt, 16) > 16);
    }

    #[test]
    fn keys_with_stats_rejects_key_that_cannot_be_materialized_as_value_bytes() {
        let mut map = BTreeMap::new();
        map.insert(vec![0u8; u16::MAX as usize], Value::U8(1));
        let err = CompoItem::map(map).unwrap().keys_with_stats().unwrap_err();
        assert_eq!(err.0, ItrErrCode::OutOfValueSize);
    }

    #[test]
    fn compo_map_rejects_nil_and_empty_keys() {
        let cap = SpaceCap::new(1);
        let mut compo = CompoItem::new_map();

        for key in [Value::Nil, Value::Bytes(vec![])] {
            let err = compo.insert(&cap, key.clone(), Value::U8(1)).unwrap_err();
            assert_eq!(err.0, ItrErrCode::CastBeKeyFail);
            assert!(matches!(compo.haskey(key.clone()), Err(ItrErr(ItrErrCode::CastBeKeyFail, _))));
            assert!(matches!(compo.itemget(key.clone()), Err(ItrErr(ItrErrCode::CastBeKeyFail, _))));
            assert!(matches!(compo.remove(key), Err(ItrErr(ItrErrCode::CastBeKeyFail, _))));
        }
    }

    #[test]
    fn pack_map_rejects_nil_and_empty_keys() {
        let cap = SpaceCap::new(1);

        for key in [Value::Nil, Value::Bytes(vec![])] {
            let mut ops = Stack::new(16);
            ops.push(key).unwrap();
            ops.push(Value::U8(1)).unwrap();
            ops.push(Value::U16(2)).unwrap();
            let err = CompoItem::pack_map(&cap, &mut ops).unwrap_err();
            assert_eq!(err.0, ItrErrCode::CastBeKeyFail);
        }
    }
}
