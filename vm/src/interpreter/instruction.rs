/////////////////////// opset ///////////////////////

fn locop_arithmetic<F>(x: &mut Value, y: &mut Value, f: F) -> VmrtErr
where
    F: FnOnce(&Value, &Value) -> VmrtRes<Value>,
{
    let (lx, ry) = Value::arithmetic_args2(x, y)?;
    let v = f(&lx, &ry)?;
    *x = v;
    Ok(())
}

/* * *   such as: v = x + y */
fn binop_arithmetic<F>(operand_stack: &mut Stack, f: F) -> VmrtErr
where
    F: FnOnce(&Value, &Value) -> VmrtRes<Value>,
{
    let mut y = operand_stack.pop()?;
    let x = operand_stack.peek()?;
    locop_arithmetic(x, &mut y, f)
}

fn locop_arithmetic3<F>(x: &mut Value, y: &mut Value, z: &mut Value, f: F) -> VmrtErr
where
    F: FnOnce(&Value, &Value, &Value) -> VmrtRes<Value>,
{
    let (lx, my, rz) = Value::arithmetic_args3(x, y, z)?;
    let v = f(&lx, &my, &rz)?;
    *x = v;
    Ok(())
}

fn triop_arithmetic<F>(operand_stack: &mut Stack, f: F) -> VmrtErr
where
    F: FnOnce(&Value, &Value, &Value) -> VmrtRes<Value>,
{
    let mut z = operand_stack.pop()?;
    let mut y = operand_stack.pop()?;
    let x = operand_stack.peek()?;
    locop_arithmetic3(x, &mut y, &mut z, f)
}

fn locop_arithmetic4<F>(x: &mut Value, y: &mut Value, z: &mut Value, w: &mut Value, f: F) -> VmrtErr
where
    F: FnOnce(&Value, &Value, &Value, &Value) -> VmrtRes<Value>,
{
    let (lx, my, rz, qw) = Value::arithmetic_args4(x, y, z, w)?;
    let v = f(&lx, &my, &rz, &qw)?;
    *x = v;
    Ok(())
}

fn quadop_arithmetic<F>(operand_stack: &mut Stack, f: F) -> VmrtErr
where
    F: FnOnce(&Value, &Value, &Value, &Value) -> VmrtRes<Value>,
{
    let mut w = operand_stack.pop()?;
    let mut z = operand_stack.pop()?;
    let mut y = operand_stack.pop()?;
    let x = operand_stack.peek()?;
    locop_arithmetic4(x, &mut y, &mut z, &mut w, f)
}

/* * *   binop_between *   such as: v = x && y */

fn locop_btw<F>(x: &mut Value, y: &mut Value, f: F) -> VmrtErr
where
    F: FnOnce(&Value, &Value) -> VmrtRes<Value>,
{
    let v = f(&x, &y)?;
    *x = v;
    Ok(())
}

fn binop_btw<F>(operand_stack: &mut Stack, f: F) -> VmrtErr
where
    F: FnOnce(&Value, &Value) -> VmrtRes<Value>,
{
    let mut y = operand_stack.pop()?;
    let x = operand_stack.peek()?;
    locop_btw(x, &mut y, f)
}

macro_rules! bitop {
    ( $x: expr, $y: expr, $op: ident ) => {
        Ok(match ($x, $y) {
            (U8(l), U8(r)) => Value::U8((*l).$op(*r)),
            (U16(l), U16(r)) => Value::U16((*l).$op(*r)),
            (U32(l), U32(r)) => Value::U32((*l).$op(*r)),
            (U64(l), U64(r)) => Value::U64((*l).$op(*r)),
            (U128(l), U128(r)) => Value::U128((*l).$op(*r)),
            (_, _) => {
                return itr_err_fmt!(
                    Arithmetic,
                    "cannot do bit ops between {:?} and {:?}",
                    $x,
                    $y
                )
            }
        })
    };
}

macro_rules! ahmtdo {
    ( $x: expr, $y: expr, $op: ident ) => {
        match ($x, $y) {
            (U8(l), U8(r)) => <u8>::$op(*l, *r).map(Value::U8),
            (U16(l), U16(r)) => <u16>::$op(*l, *r).map(Value::U16),
            (U32(l), U32(r)) => <u32>::$op(*l, *r).map(Value::U32),
            (U64(l), U64(r)) => <u64>::$op(*l, *r).map(Value::U64),
            (U128(l), U128(r)) => <u128>::$op(*l, *r).map(Value::U128),
            (_, _) => {
                return itr_err_fmt!(
                    Arithmetic,
                    "cannot do arithmetic between {:?} and {:?}",
                    $x,
                    $y
                )
            }
        }
    };
}

/////////////////////// logic ///////////////////////

fn check_failed_tip(op: &str, x: &Value, y: &Value) -> String {
    format!("arithmetic {} check failed with {:?} and {:?}", op, x, y)
}

fn check_failed_tip3(op: &str, x: &Value, y: &Value, z: &Value) -> String {
    format!(
        "arithmetic {} check failed with {:?}, {:?} and {:?}",
        op, x, y, z
    )
}

fn check_failed_tip4(op: &str, x: &Value, y: &Value, z: &Value, w: &Value) -> String {
    format!(
        "arithmetic {} check failed with {:?}, {:?}, {:?} and {:?}",
        op, x, y, z, w
    )
}

fn check_failed_tip1(op: &str, x: &Value) -> String {
    format!("arithmetic {} check failed with {:?}", op, x)
}

#[inline]
fn cast_uint_like_sample(sample: &Value, out: u128, err: impl Fn() -> ItrErr) -> VmrtRes<Value> {
    Ok(match sample {
        U8(..) => Value::U8(u8::try_from(out).map_err(|_| err())?),
        U16(..) => Value::U16(u16::try_from(out).map_err(|_| err())?),
        U32(..) => Value::U32(u32::try_from(out).map_err(|_| err())?),
        U64(..) => Value::U64(u64::try_from(out).map_err(|_| err())?),
        U128(..) => Value::U128(out),
        _ => return Err(err()),
    })
}

#[inline]
fn require_nonzero_u128(d: u128, err: impl Fn() -> ItrErr) -> VmrtRes<u128> {
    if d == 0 {
        Err(err())
    } else {
        Ok(d)
    }
}

#[inline]
fn half_up_round_u128_threshold(div: u128) -> u128 {
    div - div / 2
}

#[inline]
fn ceil_div_u128(num: u128, div: u128, err: impl Fn() -> ItrErr) -> VmrtRes<u128> {
    let quo = num / div;
    if num % div == 0 {
        Ok(quo)
    } else {
        quo.checked_add(1).ok_or_else(err)
    }
}

#[inline]
fn ceil_quot_if_rem_u128(quo: u128, rem: u128, err: impl Fn() -> ItrErr) -> VmrtRes<u128> {
    if rem == 0 {
        Ok(quo)
    } else {
        quo.checked_add(1).ok_or_else(err)
    }
}

fn lgc_and(x: &Value, y: &Value) -> VmrtRes<Value> {
    let lx = x.extract_bool()?;
    let ry = y.extract_bool()?;
    Ok(Value::bool(lx && ry))
}

fn lgc_or(x: &Value, y: &Value) -> VmrtRes<Value> {
    let lx = x.extract_bool()?;
    let ry = y.extract_bool()?;
    Ok(Value::bool(lx || ry))
}

#[allow(unused)]
fn lgc_not(x: &Value) -> VmrtRes<Value> {
    let v = x.extract_bool()?;
    Ok(Value::bool(!v))
}

fn lgc_equal_bool(x: &Value, y: &Value) -> VmrtRes<bool> {
    value_content_eq(x, y)
}

fn lgc_compare_fee(x: &Value, y: &Value, gas_extra: &GasExtra) -> usize {
    value_compare_fee(x, y, gas_extra.container_cmp_header_fee)
}

fn lgc_equal(x: &Value, y: &Value) -> VmrtRes<Value> {
    Ok(Value::bool(lgc_equal_bool(x, y)?))
}

fn lgc_not_equal(x: &Value, y: &Value) -> VmrtRes<Value> {
    Ok(Value::bool(!lgc_equal_bool(x, y)?))
}

fn lgc_ord_cmp<F>(x: &Value, y: &Value, f: F) -> VmrtRes<Value>
where
    F: FnOnce(u128, u128) -> bool,
{
    if !x.is_uint() || !y.is_uint() {
        return itr_err_fmt!(
            Arithmetic,
            "ordered compare only supports uint operands, got {:?} and {:?}",
            x,
            y
        );
    }
    let lx = x.extract_u128()?;
    let ry = y.extract_u128()?;
    Ok(Value::bool(f(lx, ry)))
}

fn lgc_less(x: &Value, y: &Value) -> VmrtRes<Value> {
    lgc_ord_cmp(x, y, |l, r| l < r)
}

fn lgc_less_equal(x: &Value, y: &Value) -> VmrtRes<Value> {
    lgc_ord_cmp(x, y, |l, r| l <= r)
}

fn lgc_greater(x: &Value, y: &Value) -> VmrtRes<Value> {
    lgc_ord_cmp(x, y, |l, r| l > r)
}

fn lgc_greater_equal(x: &Value, y: &Value) -> VmrtRes<Value> {
    lgc_ord_cmp(x, y, |l, r| l >= r)
}

fn bit_and(x: &Value, y: &Value) -> VmrtRes<Value> {
    bitop!(x, y, bitand)
}

fn bit_or(x: &Value, y: &Value) -> VmrtRes<Value> {
    bitop!(x, y, bitor)
}

fn bit_xor(x: &Value, y: &Value) -> VmrtRes<Value> {
    bitop!(x, y, bitxor)
}

fn bit_shift_overflow(op: &str, x: &Value, y: &Value) -> ItrErr {
    ItrErr::new(
        Arithmetic,
        &format!("bit {} shift overflow between {:?} and {:?}", op, x, y),
    )
}

fn bit_shl(x: &Value, y: &Value) -> VmrtRes<Value> {
    let res = match (x, y) {
        (U8(l), U8(r)) => <u8>::checked_shl(*l, *r as u32).map(Value::U8),
        (U16(l), U16(r)) => <u16>::checked_shl(*l, *r as u32).map(Value::U16),
        (U32(l), U32(r)) => <u32>::checked_shl(*l, *r as u32).map(Value::U32),
        (U64(l), U64(r)) => {
            let s = u32::try_from(*r).map_err(|_| bit_shift_overflow("left", x, y))?;
            <u64>::checked_shl(*l, s).map(Value::U64)
        }
        (U128(l), U128(r)) => {
            if *r > u32::MAX as u128 {
                return Err(bit_shift_overflow("left", x, y));
            }
            <u128>::checked_shl(*l, *r as u32).map(Value::U128)
        }
        (_, _) => return itr_err_fmt!(Arithmetic, "cannot do bit ops between {:?} and {:?}", x, y),
    };
    res.ok_or_else(|| bit_shift_overflow("left", x, y))
}

fn bit_shr(x: &Value, y: &Value) -> VmrtRes<Value> {
    let res = match (x, y) {
        (U8(l), U8(r)) => <u8>::checked_shr(*l, *r as u32).map(Value::U8),
        (U16(l), U16(r)) => <u16>::checked_shr(*l, *r as u32).map(Value::U16),
        (U32(l), U32(r)) => <u32>::checked_shr(*l, *r as u32).map(Value::U32),
        (U64(l), U64(r)) => {
            let s = u32::try_from(*r).map_err(|_| bit_shift_overflow("right", x, y))?;
            <u64>::checked_shr(*l, s).map(Value::U64)
        }
        (U128(l), U128(r)) => {
            if *r > u32::MAX as u128 {
                return Err(bit_shift_overflow("right", x, y));
            }
            <u128>::checked_shr(*l, *r as u32).map(Value::U128)
        }
        (_, _) => return itr_err_fmt!(Arithmetic, "cannot do bit ops between {:?} and {:?}", x, y),
    };
    res.ok_or_else(|| bit_shift_overflow("right", x, y))
}

/////////////////////// arithmetic ///////////////////////

macro_rules! ahmtdocheck {
    ( $x: expr, $y: expr, $op: ident, $tip: expr ) => {
        ahmtdo!($x, $y, $op).ok_or_else(|| ItrErr::new(Arithmetic, &check_failed_tip($tip, $x, $y)))
    };
}

fn add_checked(x: &Value, y: &Value) -> VmrtRes<Value> {
    ahmtdocheck!(x, y, checked_add, "add")
}

fn sub_checked(x: &Value, y: &Value) -> VmrtRes<Value> {
    ahmtdocheck!(x, y, checked_sub, "sub")
}

fn mul_checked(x: &Value, y: &Value) -> VmrtRes<Value> {
    ahmtdocheck!(x, y, checked_mul, "mul")
}

fn div_checked(x: &Value, y: &Value) -> VmrtRes<Value> {
    ahmtdocheck!(x, y, checked_div, "div")
}

fn mod_checked(x: &Value, y: &Value) -> VmrtRes<Value> {
    ahmtdocheck!(x, y, checked_rem, "mod") // rem = mod
}

#[inline(always)]
fn add_mod_u128(a: u128, b: u128, m: u128) -> u128 {
    let lhs = a % m;
    let rhs = b % m;
    let gap = m - rhs;
    if lhs >= gap {
        lhs - gap
    } else {
        lhs + rhs
    }
}

fn mul_mod_u128(a: u128, b: u128, m: u128) -> u128 {
    let mut lhs = a % m;
    let mut rhs = b % m;
    let mut out = 0u128;
    while rhs != 0 {
        if rhs & 1 == 1 {
            out = add_mod_u128(out, lhs, m);
        }
        rhs >>= 1;
        if rhs != 0 {
            lhs = add_mod_u128(lhs, lhs, m);
        }
    }
    out
}

#[inline(always)]
fn low_bits_mask(bits: u32) -> u128 {
    match bits {
        0 => 0,
        128.. => u128::MAX,
        _ => (1u128 << bits) - 1,
    }
}

#[inline(always)]
fn mul_wide_u128(a: u128, b: u128) -> (u128, u128) {
    let (lo, hi) = a.carrying_mul(b, 0);
    (hi, lo)
}

fn add_u256_u128(hi: u128, lo: u128, add: u128) -> Option<(u128, u128)> {
    let (lo, carry) = lo.overflowing_add(add);
    Some((hi.checked_add(carry as u128)?, lo))
}

fn add_u256(ahi: u128, alo: u128, bhi: u128, blo: u128) -> Option<(u128, u128)> {
    let (lo, carry) = alo.overflowing_add(blo);
    let hi = ahi.checked_add(bhi)?.checked_add(carry as u128)?;
    Some((hi, lo))
}

fn sub_u256(ahi: u128, alo: u128, bhi: u128, blo: u128) -> Option<(u128, u128)> {
    let (lo, borrow) = alo.overflowing_sub(blo);
    let hi = ahi.checked_sub(bhi)?.checked_sub(borrow as u128)?;
    Some((hi, lo))
}

fn sub_u256_u128(hi: u128, lo: u128, sub: u128) -> Option<(u128, u128)> {
    sub_u256(hi, lo, 0, sub)
}

/// (x*y) ± z as u256; error if the result does not fit u128 (high limb non-zero).
fn mul_xy_addsub_z_fit_u128(
    x: u128,
    y: u128,
    z: u128,
    add_z: bool,
    err: impl Fn() -> ItrErr,
) -> VmrtRes<u128> {
    let (hi, lo) = mul_wide_u128(x, y);
    let (hi, lo) = if add_z {
        add_u256_u128(hi, lo, z).ok_or_else(|| err())?
    } else {
        sub_u256_u128(hi, lo, z).ok_or_else(|| err())?
    };
    if hi != 0 {
        return Err(err());
    }
    Ok(lo)
}

/// Truncating ((x*y) ± z) / div; div must be non-zero.
fn mul_xy_addsub_z_div_u128(
    x: u128,
    y: u128,
    z: u128,
    div: u128,
    add_z: bool,
    err: impl Fn() -> ItrErr,
) -> VmrtRes<u128> {
    let (hi, lo) = mul_wide_u128(x, y);
    let (hi, lo) = if add_z {
        add_u256_u128(hi, lo, z).ok_or_else(|| err())?
    } else {
        sub_u256_u128(hi, lo, z).ok_or_else(|| err())?
    };
    let (quo, _) = div_u256_by_u128_to_u128(hi, lo, div).ok_or_else(|| err())?;
    Ok(quo)
}

fn div_u256_by_u128_to_u128(hi: u128, lo: u128, d: u128) -> Option<(u128, u128)> {
    if d == 0 || hi >= d {
        return None;
    }
    let mut rem = hi;
    let mut quo = 0u128;
    for shift in (0..128).rev() {
        let carry = rem >> 127;
        rem = (rem << 1) | ((lo >> shift) & 1);
        if carry != 0 || rem >= d {
            rem = rem.wrapping_sub(d);
            quo |= 1u128 << shift;
        }
    }
    Some((quo, rem))
}

fn div_u256_by_u129_to_u128(
    n_hi: u128,
    n_lo: u128,
    d_hi: u128,
    d_lo: u128,
) -> Option<(u128, u128, u128)> {
    if d_hi > 1 || (d_hi == 0 && d_lo == 0) {
        return None;
    }
    if d_hi == 0 {
        let (quo, rem) = div_u256_by_u128_to_u128(n_hi, n_lo, d_lo)?;
        return Some((quo, 0, rem));
    }
    let mut quo = 0u128;
    let mut rem_hi = 0u128;
    let mut rem_lo = 0u128;
    for shift in (0..256).rev() {
        let next_bit = if shift >= 128 {
            (n_hi >> (shift - 128)) & 1
        } else {
            (n_lo >> shift) & 1
        };
        let carry = rem_lo >> 127;
        rem_hi = (rem_hi << 1) | carry;
        rem_lo = (rem_lo << 1) | next_bit;
        if cmp_u256(rem_hi, rem_lo, d_hi, d_lo).is_ge() {
            let (new_hi, new_lo) = sub_u256(rem_hi, rem_lo, d_hi, d_lo)?;
            rem_hi = new_hi;
            rem_lo = new_lo;
            if shift >= 128 {
                return None;
            }
            quo |= 1u128 << shift;
        }
    }
    Some((quo, rem_hi, rem_lo))
}

fn shr_u256_to_u128(hi: u128, lo: u128, shift: u32) -> Option<(u128, bool)> {
    match shift {
        0 => {
            if hi == 0 {
                Some((lo, false))
            } else {
                None
            }
        }
        1..=127 => {
            if hi >> shift != 0 {
                return None;
            }
            let out = (hi << (128 - shift)) | (lo >> shift);
            let dropped = lo & low_bits_mask(shift) != 0;
            Some((out, dropped))
        }
        128 => Some((hi, lo != 0)),
        129..=255 => {
            let rhs = shift - 128;
            let out = hi >> rhs;
            let dropped = lo != 0 || hi & low_bits_mask(rhs) != 0;
            Some((out, dropped))
        }
        _ => None,
    }
}

fn mul_u256_u128_to_u256_checked(hi: u128, lo: u128, mul: u128) -> Option<(u128, u128)> {
    let (lo_hi, lo_lo) = mul_wide_u128(lo, mul);
    let (hi_hi, hi_lo) = mul_wide_u128(hi, mul);
    if hi_hi != 0 {
        return None;
    }
    let out_hi = hi_lo.checked_add(lo_hi)?;
    Some((out_hi, lo_lo))
}

fn cmp_u256(ahi: u128, alo: u128, bhi: u128, blo: u128) -> std::cmp::Ordering {
    ahi.cmp(&bhi).then(alo.cmp(&blo))
}

fn cast_uint_result1(sample: &Value, out: u128, op: &str, x: &Value) -> VmrtRes<Value> {
    cast_uint_like_sample(sample, out, || ItrErr::new(Arithmetic, &check_failed_tip1(op, x)))
}

fn cast_uint_result2(sample: &Value, out: u128, op: &str, x: &Value, y: &Value) -> VmrtRes<Value> {
    cast_uint_like_sample(sample, out, || ItrErr::new(Arithmetic, &check_failed_tip(op, x, y)))
}

fn cast_uint_result3(
    sample: &Value,
    out: u128,
    op: &str,
    x: &Value,
    y: &Value,
    z: &Value,
) -> VmrtRes<Value> {
    cast_uint_like_sample(sample, out, || ItrErr::new(Arithmetic, &check_failed_tip3(op, x, y, z)))
}

fn cast_uint_result4(
    sample: &Value,
    out: u128,
    op: &str,
    x: &Value,
    y: &Value,
    z: &Value,
    w: &Value,
) -> VmrtRes<Value> {
    cast_uint_like_sample(sample, out, || ItrErr::new(Arithmetic, &check_failed_tip4(op, x, y, z, w)))
}

fn round_half_up_div_u256_by_u128(
    hi: u128,
    lo: u128,
    d: u128,
    op: &str,
    x: &Value,
    y: &Value,
    z: &Value,
) -> VmrtRes<u128> {
    let err = || ItrErr::new(Arithmetic, &check_failed_tip3(op, x, y, z));
    let (mut quo, rem) = div_u256_by_u128_to_u128(hi, lo, d).ok_or_else(err)?;
    let threshold = half_up_round_u128_threshold(d);
    if rem >= threshold {
        quo = quo.checked_add(1).ok_or_else(err)?;
    }
    Ok(quo)
}

fn mul_div_half_up(
    lhs: u128,
    rhs: u128,
    d: u128,
    op: &str,
    x: &Value,
    y: &Value,
    z: &Value,
) -> VmrtRes<u128> {
    let (hi, lo) = mul_wide_u128(lhs, rhs);
    round_half_up_div_u256_by_u128(hi, lo, d, op, x, y, z)
}

fn weighted_sum_div_u128(
    lhs: u128,
    lhs_weight: u128,
    rhs: u128,
    rhs_weight: u128,
    denom: u128,
) -> Option<(u128, u128)> {
    if denom == 0 {
        return None;
    }
    let (lhs_hi, lhs_lo) = mul_wide_u128(lhs, lhs_weight);
    let (rhs_hi, rhs_lo) = mul_wide_u128(rhs, rhs_weight);
    let (num_hi, num_lo) = add_u256(lhs_hi, lhs_lo, rhs_hi, rhs_lo)?;
    div_u256_by_u128_to_u128(num_hi, num_lo, denom)
}

fn scaled_abs_diff_div_u128(x: u128, reference: u128, scale: u128) -> Option<(u128, u128)> {
    if reference == 0 || scale == 0 {
        return None;
    }
    let diff = x.abs_diff(reference);
    let (hi, lo) = mul_wide_u128(diff, scale);
    div_u256_by_u128_to_u128(hi, lo, reference)
}

fn saturating_uint_add(x: &Value, y: &Value) -> VmrtRes<Value> {
    Ok(match (x, y) {
        (U8(l), U8(r)) => Value::U8(l.saturating_add(*r)),
        (U16(l), U16(r)) => Value::U16(l.saturating_add(*r)),
        (U32(l), U32(r)) => Value::U32(l.saturating_add(*r)),
        (U64(l), U64(r)) => Value::U64(l.saturating_add(*r)),
        (U128(l), U128(r)) => Value::U128(l.saturating_add(*r)),
        (_, _) => {
            return itr_err_fmt!(
                Arithmetic,
                "cannot do arithmetic between {:?} and {:?}",
                x,
                y
            )
        }
    })
}

fn saturating_uint_sub(x: &Value, y: &Value) -> VmrtRes<Value> {
    Ok(match (x, y) {
        (U8(l), U8(r)) => Value::U8(l.saturating_sub(*r)),
        (U16(l), U16(r)) => Value::U16(l.saturating_sub(*r)),
        (U32(l), U32(r)) => Value::U32(l.saturating_sub(*r)),
        (U64(l), U64(r)) => Value::U64(l.saturating_sub(*r)),
        (U128(l), U128(r)) => Value::U128(l.saturating_sub(*r)),
        (_, _) => {
            return itr_err_fmt!(
                Arithmetic,
                "cannot do arithmetic between {:?} and {:?}",
                x,
                y
            )
        }
    })
}

fn absdiff_checked(x: &Value, y: &Value) -> VmrtRes<Value> {
    Ok(match (x, y) {
        (U8(l), U8(r)) => Value::U8(l.abs_diff(*r)),
        (U16(l), U16(r)) => Value::U16(l.abs_diff(*r)),
        (U32(l), U32(r)) => Value::U32(l.abs_diff(*r)),
        (U64(l), U64(r)) => Value::U64(l.abs_diff(*r)),
        (U128(l), U128(r)) => Value::U128(l.abs_diff(*r)),
        (_, _) => {
            return itr_err_fmt!(
                Arithmetic,
                "cannot do arithmetic between {:?} and {:?}",
                x,
                y
            )
        }
    })
}

fn sqrt_floor_checked(x: &Value) -> VmrtRes<Value> {
    let n = x.extract_u128()?;
    let out = n.isqrt();
    cast_uint_result1(x, out, "sqrt", x)
}

fn sqrt_up_checked(x: &Value) -> VmrtRes<Value> {
    let n = x.extract_u128()?;
    let err = || ItrErr::new(Arithmetic, &check_failed_tip1("sqrt_up", x));
    let f = n.isqrt();
    let out = if n <= 1 {
        n
    } else if f.checked_mul(f) == Some(n) {
        f
    } else {
        f.checked_add(1).ok_or_else(err)?
    };
    cast_uint_result1(x, out, "sqrt_up", x)
}

fn addmod_checked(x: &Value, y: &Value, z: &Value) -> VmrtRes<Value> {
    let err = || ItrErr::new(Arithmetic, &check_failed_tip3("add_mod", x, y, z));
    let a = x.extract_u128()?;
    let b = y.extract_u128()?;
    let modu = require_nonzero_u128(z.extract_u128()?, err)?;
    let out = add_mod_u128(a, b, modu);
    cast_uint_result3(x, out, "add_mod", x, y, z)
}

fn mulmod_checked(x: &Value, y: &Value, z: &Value) -> VmrtRes<Value> {
    let err = || ItrErr::new(Arithmetic, &check_failed_tip3("mul_mod", x, y, z));
    let modu = require_nonzero_u128(z.extract_u128()?, err)?;
    let out = mul_mod_u128(x.extract_u128()?, y.extract_u128()?, modu);
    cast_uint_result3(x, out, "mul_mod", x, y, z)
}

fn muldiv_checked(x: &Value, y: &Value, z: &Value) -> VmrtRes<Value> {
    let err = || ItrErr::new(Arithmetic, &check_failed_tip3("mul_div", x, y, z));
    let div = require_nonzero_u128(z.extract_u128()?, err)?;
    let (hi, lo) = mul_wide_u128(x.extract_u128()?, y.extract_u128()?);
    let (quo, _) = div_u256_by_u128_to_u128(hi, lo, div).ok_or_else(err)?;
    cast_uint_result3(x, quo, "mul_div", x, y, z)
}

fn muladd_checked(x: &Value, y: &Value, z: &Value) -> VmrtRes<Value> {
    let err = || ItrErr::new(Arithmetic, &check_failed_tip3("mul_add", x, y, z));
    let lo = mul_xy_addsub_z_fit_u128(
        x.extract_u128()?,
        y.extract_u128()?,
        z.extract_u128()?,
        true,
        err,
    )?;
    cast_uint_result3(x, lo, "mul_add", x, y, z)
}

fn muldivup_checked(x: &Value, y: &Value, z: &Value) -> VmrtRes<Value> {
    let err = || ItrErr::new(Arithmetic, &check_failed_tip3("mul_div_up", x, y, z));
    let div = require_nonzero_u128(z.extract_u128()?, err)?;
    let (hi, lo) = mul_wide_u128(x.extract_u128()?, y.extract_u128()?);
    let (quo, rem) = div_u256_by_u128_to_u128(hi, lo, div).ok_or_else(err)?;
    let quo = ceil_quot_if_rem_u128(quo, rem, err)?;
    cast_uint_result3(x, quo, "mul_div_up", x, y, z)
}

fn mul_shr_impl(x: &Value, y: &Value, z: &Value, op: &'static str, ceil_dropped: bool) -> VmrtRes<Value> {
    let err = || ItrErr::new(Arithmetic, &check_failed_tip3(op, x, y, z));
    let shift = z.extract_u128()?;
    let (hi, lo) = mul_wide_u128(x.extract_u128()?, y.extract_u128()?);
    let out = if shift >= 256 {
        if ceil_dropped && (hi != 0 || lo != 0) { 1 } else { 0 }
    } else {
        let (mut out, dropped) = shr_u256_to_u128(hi, lo, shift as u32).ok_or_else(err)?;
        if ceil_dropped && dropped {
            out = out.checked_add(1).ok_or_else(err)?;
        }
        out
    };
    cast_uint_result3(x, out, op, x, y, z)
}

fn mulshr_checked(x: &Value, y: &Value, z: &Value) -> VmrtRes<Value> {
    mul_shr_impl(x, y, z, "mul_shr", false)
}

fn mulshrup_checked(x: &Value, y: &Value, z: &Value) -> VmrtRes<Value> {
    mul_shr_impl(x, y, z, "mul_shr_up", true)
}

fn rpow_checked(x: &Value, y: &Value, z: &Value) -> VmrtRes<Value> {
    let err = || ItrErr::new(Arithmetic, &check_failed_tip3("rpow", x, y, z));
    let mut n = y.extract_u128()?;
    let base = z.extract_u128()?;
    if base == 0 {
        return Err(err());
    }
    if n == 0 {
        return cast_uint_result3(x, base, "rpow", x, y, z);
    }
    let mut bas = x.extract_u128()?;
    let mut out = if n & 1 == 1 { bas } else { base };
    while n > 1 {
        n >>= 1;
        bas = mul_div_half_up(bas, bas, base, "rpow", x, y, z)?;
        if n & 1 == 1 {
            out = mul_div_half_up(out, bas, base, "rpow", x, y, z)?;
        }
    }
    cast_uint_result3(x, out, "rpow", x, y, z)
}

fn clamp_checked(x: &Value, y: &Value, z: &Value) -> VmrtRes<Value> {
    let err = || ItrErr::new(Arithmetic, &check_failed_tip3("clamp", x, y, z));
    let xv = x.extract_u128()?;
    let lo = y.extract_u128()?;
    let hi = z.extract_u128()?;
    if lo > hi {
        return Err(err());
    }
    let out = xv.clamp(lo, hi);
    cast_uint_result3(x, out, "clamp", x, y, z)
}

fn satadd_checked(x: &Value, y: &Value) -> VmrtRes<Value> {
    saturating_uint_add(x, y)
}

fn satsub_checked(x: &Value, y: &Value) -> VmrtRes<Value> {
    saturating_uint_sub(x, y)
}

fn divup_checked(x: &Value, y: &Value) -> VmrtRes<Value> {
    let err = || ItrErr::new(Arithmetic, &check_failed_tip("div_up", x, y));
    let div = require_nonzero_u128(y.extract_u128()?, err)?;
    let num = x.extract_u128()?;
    let quo = ceil_div_u128(num, div, err)?;
    cast_uint_result2(x, quo, "div_up", x, y)
}

fn divround_checked(x: &Value, y: &Value) -> VmrtRes<Value> {
    let err = || ItrErr::new(Arithmetic, &check_failed_tip("div_round", x, y));
    let div = require_nonzero_u128(y.extract_u128()?, err)?;
    let num = x.extract_u128()?;
    let mut quo = num / div;
    let rem = num % div;
    let threshold = half_up_round_u128_threshold(div);
    if rem >= threshold {
        quo = quo.checked_add(1).ok_or_else(err)?;
    }
    cast_uint_result2(x, quo, "div_round", x, y)
}

fn mulsub_checked(x: &Value, y: &Value, z: &Value) -> VmrtRes<Value> {
    let err = || ItrErr::new(Arithmetic, &check_failed_tip3("mul_sub", x, y, z));
    let lo = mul_xy_addsub_z_fit_u128(
        x.extract_u128()?,
        y.extract_u128()?,
        z.extract_u128()?,
        false,
        err,
    )?;
    cast_uint_result3(x, lo, "mul_sub", x, y, z)
}

fn muldivround_checked(x: &Value, y: &Value, z: &Value) -> VmrtRes<Value> {
    let err = || ItrErr::new(Arithmetic, &check_failed_tip3("mul_div_round", x, y, z));
    let div = require_nonzero_u128(z.extract_u128()?, err)?;
    let (hi, lo) = mul_wide_u128(x.extract_u128()?, y.extract_u128()?);
    let quo = round_half_up_div_u256_by_u128(hi, lo, div, "mul_div_round", x, y, z)
        .map_err(|_| err())?;
    cast_uint_result3(x, quo, "mul_div_round", x, y, z)
}

fn muladddiv_checked(x: &Value, y: &Value, z: &Value, w: &Value) -> VmrtRes<Value> {
    let err = || ItrErr::new(Arithmetic, &check_failed_tip4("mul_add_div", x, y, z, w));
    let div = require_nonzero_u128(w.extract_u128()?, err)?;
    let quo = mul_xy_addsub_z_div_u128(
        x.extract_u128()?,
        y.extract_u128()?,
        z.extract_u128()?,
        div,
        true,
        err,
    )?;
    cast_uint_result4(x, quo, "mul_add_div", x, y, z, w)
}

fn mulsubdiv_checked(x: &Value, y: &Value, z: &Value, w: &Value) -> VmrtRes<Value> {
    let err = || ItrErr::new(Arithmetic, &check_failed_tip4("mul_sub_div", x, y, z, w));
    let div = require_nonzero_u128(w.extract_u128()?, err)?;
    let quo = mul_xy_addsub_z_div_u128(
        x.extract_u128()?,
        y.extract_u128()?,
        z.extract_u128()?,
        div,
        false,
        err,
    )?;
    cast_uint_result4(x, quo, "mul_sub_div", x, y, z, w)
}

fn mul3div_checked(x: &Value, y: &Value, z: &Value, w: &Value) -> VmrtRes<Value> {
    let err = || ItrErr::new(Arithmetic, &check_failed_tip4("mul3_div", x, y, z, w));
    let div = require_nonzero_u128(w.extract_u128()?, err)?;
    let (hi, lo) = mul_wide_u128(x.extract_u128()?, y.extract_u128()?);
    let (hi, lo) = mul_u256_u128_to_u256_checked(hi, lo, z.extract_u128()?).ok_or_else(err)?;
    let (quo, _) = div_u256_by_u128_to_u128(hi, lo, div).ok_or_else(err)?;
    cast_uint_result4(x, quo, "mul3_div", x, y, z, w)
}

fn devscaled_floor_checked(x: &Value, y: &Value, z: &Value) -> VmrtRes<Value> {
    let err = || ItrErr::new(Arithmetic, &check_failed_tip3("dev_scaled_floor", x, y, z));
    let (quo, _) = scaled_abs_diff_div_u128(x.extract_u128()?, y.extract_u128()?, z.extract_u128()?)
        .ok_or_else(err)?;
    cast_uint_result3(x, quo, "dev_scaled_floor", x, y, z)
}

fn devscaled_checked(x: &Value, y: &Value, z: &Value) -> VmrtRes<Value> {
    let err = || ItrErr::new(Arithmetic, &check_failed_tip3("dev_scaled", x, y, z));
    let (quo, rem) = scaled_abs_diff_div_u128(x.extract_u128()?, y.extract_u128()?, z.extract_u128()?)
        .ok_or_else(err)?;
    let out = ceil_quot_if_rem_u128(quo, rem, err)?;
    cast_uint_result3(x, out, "dev_scaled", x, y, z)
}

fn withinbps_checked(x: &Value, y: &Value, z: &Value, w: &Value) -> VmrtRes<Value> {
    let err = || ItrErr::new(Arithmetic, &check_failed_tip4("within_bps", x, y, z, w));
    let value = x.extract_u128()?;
    let reference = y.extract_u128()?;
    let tolerance = z.extract_u128()?;
    let scale = w.extract_u128()?;
    if reference == 0 || scale == 0 {
        return Err(err());
    }
    let diff = value.abs_diff(reference);
    let (lhs_hi, lhs_lo) = mul_wide_u128(diff, scale);
    let (rhs_hi, rhs_lo) = mul_wide_u128(reference, tolerance);
    Ok(Value::bool(cmp_u256(lhs_hi, lhs_lo, rhs_hi, rhs_lo).is_le()))
}

fn wavg2_checked(x: &Value, y: &Value, z: &Value, w: &Value) -> VmrtRes<Value> {
    let err = || ItrErr::new(Arithmetic, &check_failed_tip4("wavg2", x, y, z, w));
    let lhs = x.extract_u128()?;
    let rhs = z.extract_u128()?;
    let wx = y.extract_u128()?;
    let wy = w.extract_u128()?;
    let (den_hi, den_lo) = add_u256_u128(0, wx, wy).ok_or_else(err)?;
    if den_hi == 0 && den_lo == 0 {
        return Err(err());
    }
    if lhs == rhs {
        return cast_uint_result4(x, lhs, "wavg2", x, y, z, w);
    }
    let (base, diff, diff_weight) = if lhs < rhs {
        (lhs, rhs - lhs, wy)
    } else {
        (rhs, lhs - rhs, wx)
    };
    let (part_hi, part_lo) = mul_wide_u128(diff, diff_weight);
    let (part, _, _) = div_u256_by_u129_to_u128(part_hi, part_lo, den_hi, den_lo).ok_or_else(err)?;
    let out = base.checked_add(part).ok_or_else(err)?;
    cast_uint_result4(x, out, "wavg2", x, y, z, w)
}

fn lerp_checked(x: &Value, y: &Value, z: &Value, w: &Value) -> VmrtRes<Value> {
    let err = || ItrErr::new(Arithmetic, &check_failed_tip4("lerp", x, y, z, w));
    let start = x.extract_u128()?;
    let end = y.extract_u128()?;
    let weight = z.extract_u128()?;
    let base = w.extract_u128()?;
    if base == 0 || weight > base {
        return Err(err());
    }
    let left_weight = base - weight;
    let (quo, _) = weighted_sum_div_u128(start, left_weight, end, weight, base)
        .ok_or_else(err)?;
    cast_uint_result4(x, quo, "lerp", x, y, z, w)
}

// the value is must within u32
fn pow_checked(x: &Value, y: &Value) -> VmrtRes<Value> {
    let exp_u32 = |n: u128| -> VmrtRes<u32> {
        u32::try_from(n).map_err(|_| ItrErr::new(Arithmetic, &check_failed_tip("pow", x, y)))
    };
    match (x, y) {
        (U8(l), U8(r)) => <u8>::checked_pow(*l, *r as u32).map(Value::U8),
        (U16(l), U16(r)) => <u16>::checked_pow(*l, *r as u32).map(Value::U16),
        (U32(l), U32(r)) => <u32>::checked_pow(*l, *r).map(Value::U32),
        (U64(l), U64(r)) => <u64>::checked_pow(*l, exp_u32(*r as u128)?).map(Value::U64),
        (U128(l), U128(r)) => <u128>::checked_pow(*l, exp_u32(*r)?).map(Value::U128),
        (_, _) => {
            return itr_err_fmt!(
                Arithmetic,
                "cannot do pow arithmetic between {:?} and {:?}",
                x,
                y
            );
        }
    }
    .ok_or_else(|| ItrErr::new(Arithmetic, &check_failed_tip("pow", x, y)))
}

fn max_checked(x: &Value, y: &Value) -> VmrtRes<Value> {
    let a = x.extract_u128()?;
    let b = y.extract_u128()?;
    Ok(maybe!(a > b, x.clone(), y.clone()))
}

fn min_checked(x: &Value, y: &Value) -> VmrtRes<Value> {
    let a = x.extract_u128()?;
    let b = y.extract_u128()?;
    Ok(maybe!(a < b, x.clone(), y.clone()))
}

fn unary_inc(x: &mut Value, n: u8) -> VmrtErr {
    if !x.is_uint() {
        return itr_err_fmt!(Arithmetic, "cannot do arithmetic with {:?}", x);
    }
    x.inc(n)
        .map_err(|ItrErr(_, msg)| ItrErr::new(Arithmetic, &msg))
}

fn unary_dec(x: &mut Value, n: u8) -> VmrtErr {
    if !x.is_uint() {
        return itr_err_fmt!(Arithmetic, "cannot do arithmetic with {:?}", x);
    }
    x.dec(n)
        .map_err(|ItrErr(_, msg)| ItrErr::new(Arithmetic, &msg))
}

#[cfg(test)]
mod shift_u64_tests {
    use super::*;

    fn test_heap() -> Heap {
        let mut h = Heap::new(64);
        let gst = GasExtra::new(1);
        h.grow(1, &gst).unwrap();
        h
    }

    #[test]
    fn bit_shl_u64_rejects_shift_count_over_u32() {
        let r = Value::U64((u32::MAX as u64) + 1);
        assert!(bit_shl(&Value::U64(1), &r).is_err());
    }

    #[test]
    fn bit_shr_u64_rejects_shift_count_over_u32() {
        let r = Value::U64((u32::MAX as u64) + 1);
        assert!(bit_shr(&Value::U64(1), &r).is_err());
    }

    #[test]
    fn logic_and_or_auto_cast_to_bool() {
        assert_eq!(
            lgc_and(&Value::U8(2), &Value::Bytes(vec![])).unwrap(),
            Value::Bool(false)
        );
        assert_eq!(
            lgc_or(&Value::U8(2), &Value::Bytes(vec![])).unwrap(),
            Value::Bool(true)
        );
    }

    #[test]
    fn eq_neq_uint_operands_use_numeric_compare() {
        assert_eq!(
            lgc_equal(&Value::U8(1), &Value::U16(1)).unwrap(),
            Value::Bool(true)
        );
        assert_eq!(
            lgc_not_equal(&Value::U32(5), &Value::U8(4)).unwrap(),
            Value::Bool(true)
        );
        assert_eq!(
            lgc_not_equal(&Value::U128(0), &Value::U8(0)).unwrap(),
            Value::Bool(false)
        );
    }

    #[test]
    fn eq_neq_reject_different_non_uint_types() {
        let adr = field::Address::from_readable("1MzNY1oA3kfgYi75zquj3SRUPYztzXHzK9").unwrap();
        assert!(lgc_equal(&Value::U16(1), &Value::Bytes(vec![1])).is_err());
        assert!(lgc_equal(&Value::U8(1), &Value::Bool(true)).is_err());
        assert!(lgc_equal(&Value::Nil, &Value::Bool(false)).is_err());
        assert!(lgc_not_equal(&Value::Address(adr), &Value::Bytes(vec![])).is_err());
    }

    #[test]
    fn eq_non_uint_same_type_compare_by_type_rules() {
        assert_eq!(
            lgc_equal(&Value::Bytes(vec![0, 1]), &Value::Bytes(vec![1])).unwrap(),
            Value::Bool(false)
        );
        assert_eq!(
            lgc_equal(&Value::Bytes(vec![1]), &Value::Bytes(vec![1])).unwrap(),
            Value::Bool(true)
        );
        assert_eq!(
            lgc_equal(&Value::Bool(true), &Value::Bool(true)).unwrap(),
            Value::Bool(true)
        );
        assert_eq!(
            lgc_equal(&Value::Nil, &Value::Nil).unwrap(),
            Value::Bool(true)
        );
    }

    #[test]
    fn ordered_compare_requires_uint_operands() {
        assert_eq!(
            lgc_less(&Value::U8(1), &Value::U16(2)).unwrap(),
            Value::Bool(true)
        );
        assert_eq!(
            lgc_greater_equal(&Value::U32(2), &Value::U8(2)).unwrap(),
            Value::Bool(true)
        );
        assert!(lgc_less(&Value::Bytes(vec![1]), &Value::U16(2)).is_err());
        assert!(lgc_greater_equal(&Value::Bool(false), &Value::U8(1)).is_err());
        assert!(lgc_less_equal(&Value::Nil, &Value::U8(0)).is_err());
        assert!(lgc_greater(&Value::Bytes(vec![2]), &Value::Bytes(vec![1])).is_err());
    }

    #[test]
    fn pow_supports_u64_u128_and_rejects_exp_over_u32() {
        assert_eq!(
            pow_checked(&Value::U64(2), &Value::U64(10)).unwrap(),
            Value::U64(1024)
        );
        assert_eq!(
            pow_checked(&Value::U128(2), &Value::U128(10)).unwrap(),
            Value::U128(1024)
        );
        let over = Value::U128((u32::MAX as u128) + 1);
        assert!(pow_checked(&Value::U128(2), &over).is_err());
    }

    #[test]
    fn max_min_mixed_uint_widths_return_normalized_type() {
        let mut lhs_max = Value::U8(255);
        let mut rhs_max = Value::U128(254);
        locop_arithmetic(&mut lhs_max, &mut rhs_max, max_checked).unwrap();
        assert_eq!(lhs_max, Value::U128(255));

        let mut lhs_min = Value::U16(1);
        let mut rhs_min = Value::U64(2);
        locop_arithmetic(&mut lhs_min, &mut rhs_min, min_checked).unwrap();
        assert_eq!(lhs_min, Value::U64(1));
    }

    #[test]
    fn heapslice_eq_is_rejected_and_other_ops_still_reject() {
        let mut heap = test_heap();
        heap.write(0, Value::Bytes(vec![1, 2, 3])).unwrap();
        let hs = Value::HeapSlice((0, 2));

        assert!(Value::arithmetic_args2(&hs, &Value::U8(1)).is_err());
        assert!(lgc_equal(&hs, &Value::HeapSlice((0, 2))).is_err());
        assert!(lgc_not_equal(&hs, &Value::HeapSlice((0, 3))).is_err());
        assert!(lgc_equal(&hs, &Value::Bytes(vec![1, 2])).is_err());
        assert!(lgc_not_equal(&hs, &Value::Bytes(vec![1, 2])).is_err());
        assert!(lgc_less(&hs, &Value::U8(1)).is_err());

        let mut incv = hs.clone();
        let mut decv = hs.clone();
        assert!(unary_inc(&mut incv, 1).is_err());
        assert!(unary_dec(&mut decv, 1).is_err());
    }

    #[test]
    fn tuple_eq_uses_content_semantics() {
        let shared = TupleItem::new(vec![Value::U8(7), Value::Bytes(vec![1, 2, 3])]).unwrap();
        let same = shared.clone();
        let rebuilt = TupleItem::new(vec![Value::U8(7), Value::Bytes(vec![1, 2, 3])]).unwrap();
        let diff = TupleItem::new(vec![Value::U8(7), Value::Bytes(vec![1, 2, 4])]).unwrap();

        assert_eq!(
            lgc_equal(&Value::Tuple(shared.clone()), &Value::Tuple(same)).unwrap(),
            Value::Bool(true)
        );
        assert_eq!(
            lgc_equal(&Value::Tuple(shared.clone()), &Value::Tuple(rebuilt)).unwrap(),
            Value::Bool(true)
        );
        assert_eq!(
            lgc_equal(&Value::Tuple(shared.clone()), &Value::Tuple(diff)).unwrap(),
            Value::Bool(false)
        );
    }

    #[test]
    fn compo_eq_uses_content_semantics() {
        let c = CompoItem::list(VecDeque::from([Value::U8(1), Value::Bytes(vec![2, 3])])).unwrap();
        let same = c.clone();
        let copied = CompoItem::list(VecDeque::from([Value::U8(1), Value::Bytes(vec![2, 3])])).unwrap();
        let diff = CompoItem::list(VecDeque::from([Value::U8(1), Value::Bytes(vec![2, 4])])).unwrap();

        assert_eq!(
            lgc_equal(&Value::Compo(c.clone()), &Value::Compo(same)).unwrap(),
            Value::Bool(true)
        );
        assert_eq!(
            lgc_equal(&Value::Compo(c.clone()), &Value::Compo(copied.clone())).unwrap(),
            Value::Bool(true)
        );
        assert_eq!(
            lgc_equal(&Value::Compo(c), &Value::Compo(diff)).unwrap(),
            Value::Bool(false)
        );
        assert!(lgc_equal(&Value::Compo(copied.clone()), &Value::Nil).is_err());
        assert!(lgc_not_equal(&Value::Compo(copied), &Value::Nil).is_err());
    }

    #[test]
    fn handle_eq_is_rejected() {
        let lhs = Value::handle(7u32);
        let rhs = lhs.clone();
        assert!(lgc_equal(&lhs, &rhs).is_err());
        assert!(lgc_not_equal(&lhs, &rhs).is_err());
    }

    #[test]
    fn finance_rounding_and_saturating_ops_work() {
        assert_eq!(
            divup_checked(&Value::U64(10), &Value::U64(3)).unwrap(),
            Value::U64(4)
        );
        assert_eq!(
            divround_checked(&Value::U64(10), &Value::U64(6)).unwrap(),
            Value::U64(2)
        );
        assert_eq!(
            satadd_checked(&Value::U8(u8::MAX), &Value::U8(1)).unwrap(),
            Value::U8(u8::MAX)
        );
        assert_eq!(
            satsub_checked(&Value::U16(0), &Value::U16(7)).unwrap(),
            Value::U16(0)
        );
        assert_eq!(
            absdiff_checked(&Value::U64(3), &Value::U64(11)).unwrap(),
            Value::U64(8)
        );
    }

    #[test]
    fn finance_mul_variants_work() {
        assert_eq!(
            mulsub_checked(&Value::U64(9), &Value::U64(8), &Value::U64(6)).unwrap(),
            Value::U64(66)
        );
        assert_eq!(
            muldivround_checked(&Value::U64(5), &Value::U64(10), &Value::U64(6)).unwrap(),
            Value::U64(8)
        );
        assert_eq!(
            muladddiv_checked(
                &Value::U64(2),
                &Value::U64(3),
                &Value::U64(4),
                &Value::U64(5),
            )
            .unwrap(),
            Value::U64(2)
        );
        assert_eq!(
            mulsubdiv_checked(
                &Value::U64(9),
                &Value::U64(8),
                &Value::U64(6),
                &Value::U64(3),
            )
            .unwrap(),
            Value::U64(22)
        );
        assert_eq!(
            mul3div_checked(
                &Value::U64(3),
                &Value::U64(4),
                &Value::U64(5),
                &Value::U64(6),
            )
            .unwrap(),
            Value::U64(10)
        );
    }

    #[test]
    fn finance_guard_and_curve_ops_work() {
        assert_eq!(
            devscaled_checked(&Value::U64(10100), &Value::U64(10000), &Value::U64(10000))
                .unwrap(),
            Value::U64(100)
        );
        assert_eq!(
            devscaled_floor_checked(&Value::U64(10001), &Value::U64(10000), &Value::U64(3))
                .unwrap(),
            Value::U64(0)
        );
        assert_eq!(
            devscaled_checked(&Value::U64(10001), &Value::U64(10000), &Value::U64(3)).unwrap(),
            Value::U64(1)
        );
        assert_eq!(
            withinbps_checked(
                &Value::U64(10050),
                &Value::U64(10000),
                &Value::U64(60),
                &Value::U64(10000),
            )
            .unwrap(),
            Value::Bool(true)
        );
        assert_eq!(
            withinbps_checked(
                &Value::U64(10061),
                &Value::U64(10000),
                &Value::U64(60),
                &Value::U64(10000),
            )
            .unwrap(),
            Value::Bool(false)
        );
        assert_eq!(
            wavg2_checked(
                &Value::U64(100),
                &Value::U64(1),
                &Value::U64(300),
                &Value::U64(3),
            )
            .unwrap(),
            Value::U64(250)
        );
        assert_eq!(
            wavg2_checked(
                &Value::U64(101),
                &Value::U64(1),
                &Value::U64(100),
                &Value::U64(1),
            )
            .unwrap(),
            Value::U64(100)
        );
        assert_eq!(
            wavg2_checked(
                &Value::U64(7),
                &Value::U128(u128::MAX),
                &Value::U64(7),
                &Value::U64(1),
            )
            .unwrap(),
            Value::U64(7)
        );
        assert_eq!(
            wavg2_checked(
                &Value::U128(10),
                &Value::U128(u128::MAX),
                &Value::U128(14),
                &Value::U64(2),
            )
            .unwrap(),
            Value::U128(10)
        );
        assert_eq!(
            lerp_checked(
                &Value::U64(100),
                &Value::U64(200),
                &Value::U64(1),
                &Value::U64(4),
            )
            .unwrap(),
            Value::U64(125)
        );
        assert_eq!(
            lerp_checked(
                &Value::U64(200),
                &Value::U64(100),
                &Value::U64(1),
                &Value::U64(4),
            )
            .unwrap(),
            Value::U64(175)
        );
        assert_eq!(
            lerp_checked(&Value::U64(0), &Value::U64(1), &Value::U64(1), &Value::U64(2)).unwrap(),
            Value::U64(0)
        );
        assert_eq!(
            lerp_checked(&Value::U64(1), &Value::U64(0), &Value::U64(1), &Value::U64(2)).unwrap(),
            Value::U64(0)
        );
        assert_eq!(
            lerp_checked(&Value::U64(101), &Value::U64(100), &Value::U64(1), &Value::U64(2)).unwrap(),
            wavg2_checked(&Value::U64(101), &Value::U64(1), &Value::U64(100), &Value::U64(1))
                .unwrap()
        );
        assert_eq!(
            lerp_checked(&Value::U64(7), &Value::U64(9), &Value::U64(0), &Value::U64(5)).unwrap(),
            Value::U64(7)
        );
        assert_eq!(
            lerp_checked(&Value::U64(7), &Value::U64(9), &Value::U64(5), &Value::U64(5)).unwrap(),
            Value::U64(9)
        );
    }

    #[test]
    fn finance_guard_and_curve_ops_reject_invalid_params() {
        assert!(devscaled_checked(&Value::U64(1), &Value::U64(0), &Value::U64(10)).is_err());
        assert!(devscaled_checked(&Value::U64(1), &Value::U64(1), &Value::U64(0)).is_err());
        assert!(devscaled_floor_checked(&Value::U64(1), &Value::U64(0), &Value::U64(10)).is_err());
        assert!(devscaled_floor_checked(&Value::U64(1), &Value::U64(1), &Value::U64(0)).is_err());
        assert!(withinbps_checked(&Value::U64(1), &Value::U64(0), &Value::U64(1), &Value::U64(10)).is_err());
        assert!(withinbps_checked(&Value::U64(1), &Value::U64(1), &Value::U64(1), &Value::U64(0)).is_err());
        assert!(lerp_checked(&Value::U64(1), &Value::U64(2), &Value::U64(1), &Value::U64(0)).is_err());
        assert!(lerp_checked(&Value::U64(1), &Value::U64(2), &Value::U64(3), &Value::U64(2)).is_err());
    }

    #[test]
    fn within_bps_allows_tolerance_over_scale() {
        assert_eq!(
            withinbps_checked(&Value::U64(1), &Value::U64(1), &Value::U64(11), &Value::U64(10)).unwrap(),
            Value::Bool(true)
        );
    }

    #[test]
    fn mul_shr_large_shift_has_defined_zero_or_one_semantics() {
        assert_eq!(
            mulshr_checked(&Value::U64(3), &Value::U64(5), &Value::U16(256)).unwrap(),
            Value::U64(0)
        );
        assert_eq!(
            mulshr_checked(&Value::U64(3), &Value::U64(5), &Value::U16(300)).unwrap(),
            Value::U64(0)
        );
        assert_eq!(
            mulshrup_checked(&Value::U64(3), &Value::U64(5), &Value::U16(256)).unwrap(),
            Value::U64(1)
        );
        assert_eq!(
            mulshrup_checked(&Value::U64(0), &Value::U64(5), &Value::U16(300)).unwrap(),
            Value::U64(0)
        );
    }

    #[test]
    fn dev_scaled_matches_within_bps_thresholds() {
        let x = Value::U64(10001);
        let reference = Value::U64(10000);
        let scale = Value::U64(3);
        let tol_ok = Value::U64(1);
        let tol_bad = Value::U64(0);
        assert_eq!(
            devscaled_checked(&x, &reference, &scale).unwrap(),
            Value::U64(1)
        );
        assert_eq!(
            withinbps_checked(&x, &reference, &tol_ok, &scale).unwrap(),
            Value::Bool(true)
        );
        assert_eq!(
            withinbps_checked(&x, &reference, &tol_bad, &scale).unwrap(),
            Value::Bool(false)
        );
    }
}
