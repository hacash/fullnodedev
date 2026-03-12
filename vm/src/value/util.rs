
pub const ACTIVE_UINT_BITS: [u16; 5] = [8, 16, 32, 64, 128];
pub const ACTIVE_UINT_BYTES: [usize; 5] = [1, 2, 4, 8, 16];
pub const ACTIVE_UINT_MAX_BYTES: usize = 16;

pub const RESERVED_U256_TYPE_ID: u8 = 7;
pub const RESERVED_U256_BITS: u16 = 256;
pub const RESERVED_U256_BYTES: usize = 32;


#[inline(always)]
pub fn buf_is_empty_or_all_zero(buf: &[u8]) -> bool {
    buf.is_empty() || buf.iter().all(|&b| b == 0)
}

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
