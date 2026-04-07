
pub const ACTIVE_UINT_BITS: [u16; 5] = [8, 16, 32, 64, 128];
pub const ACTIVE_UINT_BYTES: [usize; 5] = [1, 2, 4, 8, 16];
pub const ACTIVE_UINT_MAX_BYTES: usize = 16;


#[inline(always)]
pub fn buf_is_empty_or_all_zero(buf: &[u8]) -> bool {
    buf.is_empty() || buf.iter().all(|&b| b == 0)
}

/// Canonical bool byte decoding for typed/raw byte representations only.
/// This is intentionally stricter than runtime truthiness (`Value::extract_bool`),
/// which is used by control flow and explicit `as bool` coercions.
#[inline(always)]
pub fn decode_canonical_bool_byte(byte: u8) -> Option<bool> {
    match byte {
        0 => Some(false),
        1 => Some(true),
        _ => None,
    }
}

#[inline(always)]
pub fn encode_canonical_bool_byte(value: bool) -> u8 {
    maybe!(value, 1, 0)
}

#[inline(always)]
pub fn trim_leading_zero_bytes(buf: &[u8]) -> &[u8] {
    let first_nz = buf.iter().position(|b| *b != 0).unwrap_or(buf.len());
    &buf[first_nz..]
}

#[inline(always)]
pub fn fit_be_bytes<const N: usize>(buf: &[u8]) -> Option<[u8; N]> {
    if buf.len() <= N {
        let mut out = [0u8; N];
        out[N - buf.len()..].copy_from_slice(buf);
        return Some(out);
    }
    let cut = buf.len() - N;
    if buf[..cut].iter().any(|b| *b != 0) {
        return None;
    }
    let mut out = [0u8; N];
    out.copy_from_slice(&buf[cut..]);
    Some(out)
}

#[inline(always)]
pub fn minimal_active_uint_bytes(non_zero_len: usize) -> Option<usize> {
    ACTIVE_UINT_BYTES
        .iter()
        .copied()
        .find(|w| non_zero_len <= *w)
}

#[inline(always)]
pub fn checked_value_output_len(cap: &SpaceCap, len: usize) -> VmrtRes<usize> {
    if len >= u16::MAX as usize || len > cap.value_size {
        return itr_err_code!(OutOfValueSize)
    }
    Ok(len)
}

#[inline(always)]
pub fn checked_value_output_add(cap: &SpaceCap, left: usize, right: usize) -> VmrtRes<usize> {
    let total = left
        .checked_add(right)
        .ok_or_else(|| ItrErr::code(OutOfValueSize))?;
    checked_value_output_len(cap, total)
}

pub fn buf_drop_left_zero(buf: &[u8], minl: usize) -> Vec<u8> {
    let n = buf.len();
    if n == 0 {
        return vec![]
    }
    let keep = minl.min(n);
    let trim_limit = n - keep;
    let first_non_zero = buf[..trim_limit]
        .iter()
        .position(|b| *b != 0)
        .unwrap_or(trim_limit);
    buf[first_non_zero..].into()
}

#[inline(always)]
pub fn length_value_by_len(cap: &SpaceCap, len: usize) -> VmrtRes<Value> {
    if len > cap.compo_length {
        return itr_err_code!(OutOfCompoLen)
    }
    Ok(Value::U32(len as u32))
}

#[cfg(test)]
mod value_output_len_tests {
    use super::*;

    #[test]
    fn checked_value_output_add_rejects_usize_overflow() {
        let mut cap = SpaceCap::new(1);
        cap.value_size = usize::MAX;
        let err = checked_value_output_add(&cap, usize::MAX, 1).unwrap_err();
        assert_eq!(err.0, ItrErrCode::OutOfValueSize);
    }

    #[test]
    fn checked_value_output_len_keeps_u16_limit() {
        let mut cap = SpaceCap::new(1);
        cap.value_size = usize::MAX;
        let err = checked_value_output_len(&cap, u16::MAX as usize).unwrap_err();
        assert_eq!(err.0, ItrErrCode::OutOfValueSize);
    }

    #[test]
    fn buf_drop_left_zero_trims_all_zero_buffer_when_min_is_zero() {
        assert_eq!(buf_drop_left_zero(&[0, 0, 0], 0), Vec::<u8>::new());
        assert_eq!(buf_drop_left_zero(&[0, 0, 3], 0), vec![3]);
        assert_eq!(buf_drop_left_zero(&[0, 0, 0], 2), vec![0, 0]);
    }
}
