
pub const ACTIVE_UINT_BITS: [u16; 5] = [8, 16, 32, 64, 128];
pub const ACTIVE_UINT_BYTES: [usize; 5] = [1, 2, 4, 8, 16];
pub const ACTIVE_UINT_MAX_BYTES: usize = 16;

pub const RESERVED_U256_TYPE_ID: u8 = 7;
pub const RESERVED_U256_BITS: u16 = 256;
pub const RESERVED_U256_BYTES: usize = 32;

#[inline(always)]
pub fn trim_leading_zero_bytes(buf: &[u8]) -> &[u8] {
    let first_nz = buf.iter().position(|b| *b != 0).unwrap_or(buf.len());
    &buf[first_nz..]
}

#[inline(always)]
pub fn minimal_active_uint_bytes(non_zero_len: usize) -> Option<usize> {
    ACTIVE_UINT_BYTES
        .iter()
        .copied()
        .find(|w| non_zero_len <= *w)
}

pub fn buf_drop_left_zero(buf: &[u8], minl: usize) -> Vec<u8> {
    let n = buf.len();
    if n == 0 {
        return vec![]
    }
    let mut l = 0;
    let mut m = n;
    for i in 0..n {
        l = i;
        if buf[i] != 0 || m <= minl {
            break
        }
        m -= 1;
    }
    // ok
    buf[l..].into()
}

#[inline(always)]
pub fn length_value_by_len(cap: &SpaceCap, len: usize) -> VmrtRes<Value> {
    if len > cap.compo_length {
        return itr_err_code!(OutOfCompoLen)
    }
    Ok(Value::U32(len as u32))
}

pub enum ReadList<'a> {
    Slice(&'a [Value]),
    Deque(&'a VecDeque<Value>),
}

impl ReadList<'_> {
    #[inline(always)]
    pub fn len(&self) -> usize {
        match self {
            Self::Slice(items) => items.len(),
            Self::Deque(items) => items.len(),
        }
    }

    #[inline(always)]
    fn get(&self, idx: usize) -> Option<&Value> {
        match self {
            Self::Slice(items) => items.get(idx),
            Self::Deque(items) => items.get(idx),
        }
    }

    #[inline(always)]
    pub fn length(&self, cap: &SpaceCap) -> VmrtRes<Value> {
        length_value_by_len(cap, self.len())
    }

    #[inline(always)]
    pub fn haskey(&self, k: Value) -> VmrtRes<Value> {
        let i = k.checked_u32()? as usize;
        Ok(Value::Bool(i < self.len()))
    }

    #[inline(always)]
    pub fn itemget(&self, k: Value) -> VmrtRes<Value> {
        let i = k.checked_u32()? as usize;
        match self.get(i) {
            Some(v) => Ok(v.clone()),
            None => itr_err_code!(CompoNoFindItem),
        }
    }
}
