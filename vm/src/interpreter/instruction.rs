
/////////////////////// opset ///////////////////////

fn normalize_numeric_pair(x: &Value, y: &Value) -> VmrtRes<(Value, Value)> {
    let mut lx = x.to_uint()?;
    let mut ry = y.to_uint()?;
    cast_arithmetic(&mut lx, &mut ry)?;
    Ok((lx, ry))
}

fn locop_arithmetic<F>(x: &mut Value, y: &mut Value, f: F) -> VmrtErr
where
    F: FnOnce(&Value, &Value) -> VmrtRes<Value>
{
    let (lx, ry) = normalize_numeric_pair(x, y)?;
    let v = f(&lx, &ry)?;
    *x = v;
    Ok(())
}


/* * *   such as: v = x + y */
fn binop_arithmetic<F>(operand_stack: &mut Stack, f: F) -> VmrtErr
where
    F: FnOnce(&Value, &Value) -> VmrtRes<Value>
{
    let mut y = operand_stack.pop()?;
    let x = operand_stack.peek()?;
    locop_arithmetic(x, &mut y, f)
}


/* * *   binop_between *   such as: v = x && y */

fn locop_btw<F>(x: &mut Value, y: &mut Value, f: F) -> VmrtErr
where
    F: FnOnce(&Value, &Value) -> VmrtRes<Value>
{
    let v = f(&x, &y)?;
    *x = v;
    Ok(())
}

fn binop_btw<F>(operand_stack: &mut Stack, f: F) -> VmrtErr
where
    F: FnOnce(&Value, &Value) -> VmrtRes<Value>
{
    let mut y = operand_stack.pop()?;
    let x = operand_stack.peek()?;
    locop_btw(x, &mut y, f)
}




macro_rules! bitop {
    ( $x: expr, $y: expr, $op: ident ) => {
        Ok(match ($x, $y) {
            (U8(l), U8(r))     => Value::U8((*l).$op(*r)),
            (U16(l), U16(r))   => Value::U16((*l).$op(*r)),
            (U32(l), U32(r))   => Value::U32((*l).$op(*r)),
            (U64(l), U64(r))   => Value::U64((*l).$op(*r)),
            (U128(l), U128(r)) => Value::U128((*l).$op(*r)),
            (_, _) => return itr_err_fmt!(Arithmetic, 
                "cannot do bit ops between {:?} and {:?}", $x, $y),
        })
    }
}


macro_rules! ahmtdo {
    ( $x: expr, $y: expr, $op: ident ) => {
        match ($x, $y) {
            (U8(l), U8(r))     => <u8>::$op(*l, *r).map(Value::U8),
            (U16(l), U16(r))   => <u16>::$op(*l, *r).map(Value::U16),
            (U32(l), U32(r))   => <u32>::$op(*l, *r).map(Value::U32),
            (U64(l), U64(r))   => <u64>::$op(*l, *r).map(Value::U64),
            (U128(l), U128(r)) => <u128>::$op(*l, *r).map(Value::U128),
            (_, _) => return itr_err_fmt!(Arithmetic, 
                "cannot do arithmetic between {:?} and {:?}", $x, $y),
        }
    }
}


/////////////////////// logic ///////////////////////




fn check_failed_tip(op: &str, x: &Value, y: &Value) -> String {
    format!("arithmetic {} check failed with {:?} and {:?}", op, x, y)
}



fn lgc_and(x: &Value, y: &Value) -> VmrtRes<Value> {
    let lx = x.canbe_bool()?;
    let ry = y.canbe_bool()?;
    Ok(Value::bool(lx && ry))
}

fn lgc_or(x: &Value, y: &Value) -> VmrtRes<Value> {
    let lx = x.canbe_bool()?;
    let ry = y.canbe_bool()?;
    Ok(Value::bool(lx || ry))
}

#[allow(unused)]
fn lgc_not(x: &Value) -> VmrtRes<Value> {
    let v = x.canbe_bool()?;
    Ok(Value::bool(!v))
}

fn lgc_equal_bool(x: &Value, y: &Value) -> VmrtRes<bool> {
    if x.is_uint() && y.is_uint() {
        return Ok(x.checked_u128()? == y.checked_u128()?)
    }
    if x.ty() != y.ty() {
        return itr_err_fmt!(Arithmetic,
            "cannot compare different types {:?} and {:?}", x, y)
    }
    match (x, y) {
        (Nil, Nil) => Ok(true),
        (Bool(l), Bool(r)) => Ok(l == r),
        (Bytes(l), Bytes(r)) => Ok(l == r),
        (Address(l), Address(r)) => Ok(l == r),
        (HeapSlice(l), HeapSlice(r)) => Ok(l == r),
        (Args(l), Args(r)) => Ok(l == r),
        (Compo(l), Compo(r)) => Ok(l.ptr_eq(r)),
        (U8(l), U8(r)) => Ok(l == r),
        (U16(l), U16(r)) => Ok(l == r),
        (U32(l), U32(r)) => Ok(l == r),
        (U64(l), U64(r)) => Ok(l == r),
        (U128(l), U128(r)) => Ok(l == r),
        _ => itr_err_fmt!(Arithmetic, "cannot compare {:?} and {:?}", x, y),
    }
}

fn lgc_equal(x: &Value, y: &Value) -> VmrtRes<Value> {
    Ok(Value::bool(lgc_equal_bool(x, y)?))
}

fn lgc_not_equal(x: &Value, y: &Value) -> VmrtRes<Value> {
    Ok(Value::bool(!lgc_equal_bool(x, y)?))
}

fn lgc_ord_cmp<F>(x: &Value, y: &Value, f: F) -> VmrtRes<Value>
where
    F: FnOnce(u128, u128) -> bool
{
    if !x.is_uint() || !y.is_uint() {
        return itr_err_fmt!(Arithmetic,
            "ordered compare only supports uint operands, got {:?} and {:?}", x, y)
    }
    let lx = x.checked_u128()?;
    let ry = y.checked_u128()?;
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
    ItrErr::new(Arithmetic, &format!("bit {} shift overflow between {:?} and {:?}", op, x, y))
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
                return Err(bit_shift_overflow("left", x, y))
            }
            <u128>::checked_shl(*l, *r as u32).map(Value::U128)
        }
        (_, _) => return itr_err_fmt!(Arithmetic, 
            "cannot do bit ops between {:?} and {:?}", x, y),
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
                return Err(bit_shift_overflow("right", x, y))
            }
            <u128>::checked_shr(*l, *r as u32).map(Value::U128)
        }
        (_, _) => return itr_err_fmt!(Arithmetic, 
            "cannot do bit ops between {:?} and {:?}", x, y),
    };
    res.ok_or_else(|| bit_shift_overflow("right", x, y))
}



/////////////////////// arithmetic ///////////////////////



macro_rules! ahmtdocheck {
    ( $x: expr, $y: expr, $op: ident, $tip: expr ) => {
        ahmtdo!($x, $y, $op)
        .ok_or_else(||ItrErr::new(Arithmetic, &check_failed_tip($tip, $x, $y)))
    }
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

// the value is must within u32
fn pow_checked(x: &Value, y: &Value) -> VmrtRes<Value> {
    let exp_u32 = |n: u128| -> VmrtRes<u32> {
        u32::try_from(n).map_err(|_| ItrErr::new(Arithmetic, &check_failed_tip("pow", x, y)))
    };
    match (x, y) {
        (U8(l), U8(r))   => <u8>::checked_pow(*l, *r as u32).map(Value::U8),
        (U16(l), U16(r)) => <u16>::checked_pow(*l, *r as u32).map(Value::U16),
        (U32(l), U32(r)) => <u32>::checked_pow(*l, *r).map(Value::U32),
        (U64(l), U64(r)) => <u64>::checked_pow(*l, exp_u32(*r as u128)?).map(Value::U64),
        (U128(l), U128(r)) => <u128>::checked_pow(*l, exp_u32(*r)?).map(Value::U128),
        (_, _) => return itr_err_fmt!(Arithmetic, 
            "cannot do pow arithmetic between {:?} and {:?}", x, y),
    }.ok_or_else(||ItrErr::new(Arithmetic, &check_failed_tip("pow", x, y)))
}

fn max_checked(x: &Value, y: &Value) -> VmrtRes<Value> {
    let a = x.checked_u128()?;
    let b = y.checked_u128()?;
    Ok(maybe!(a > b, x.clone(), y.clone()))
}


fn min_checked(x: &Value, y: &Value) -> VmrtRes<Value> {
    let a = x.checked_u128()?;
    let b = y.checked_u128()?;
    Ok(maybe!(a < b, x.clone(), y.clone()))
}

fn unary_inc(x: &mut Value, n: u8) -> VmrtErr {
    if !x.is_uint() {
        let v = x.to_uint()?;
        *x = v;
    }
    x.inc(n).map_err(|ItrErr(_, msg)| ItrErr::new(Arithmetic, &msg))
}

fn unary_dec(x: &mut Value, n: u8) -> VmrtErr {
    if !x.is_uint() {
        let v = x.to_uint()?;
        *x = v;
    }
    x.dec(n).map_err(|ItrErr(_, msg)| ItrErr::new(Arithmetic, &msg))
}

#[cfg(test)]
mod shift_u64_tests {
    use super::*;

    fn test_heap() -> Heap {
        let mut h = Heap::new(64);
        h.grow(1).unwrap();
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
    fn heapslice_eq_uses_src_len_and_other_ops_still_reject() {
        let mut heap = test_heap();
        heap.write(0, Value::Bytes(vec![1, 2, 3])).unwrap();
        let hs = Value::HeapSlice((0, 2));

        assert!(normalize_numeric_pair(&hs, &Value::U8(1)).is_err());
        assert_eq!(lgc_equal(&hs, &Value::HeapSlice((0, 2))).unwrap(), Value::Bool(true));
        assert_eq!(lgc_equal(&hs, &Value::HeapSlice((0, 3))).unwrap(), Value::Bool(false));
        assert!(lgc_equal(&hs, &Value::Bytes(vec![1, 2])).is_err());
        assert!(lgc_not_equal(&hs, &Value::Bytes(vec![1, 2])).is_err());
        assert!(lgc_less(&hs, &Value::U8(1)).is_err());

        let mut incv = hs.clone();
        let mut decv = hs.clone();
        assert!(unary_inc(&mut incv, 1).is_err());
        assert!(unary_dec(&mut decv, 1).is_err());
    }

    #[test]
    fn compo_eq_uses_pointer_identity() {
        let c = CompoItem::new_list();
        let same = c.clone();
        let copied = c.copy();

        assert_eq!(
            lgc_equal(&Value::Compo(c.clone()), &Value::Compo(same)).unwrap(),
            Value::Bool(true)
        );
        assert_eq!(
            lgc_equal(&Value::Compo(c), &Value::Compo(copied.clone())).unwrap(),
            Value::Bool(false)
        );
        assert!(lgc_equal(&Value::Compo(copied.clone()), &Value::Nil).is_err());
        assert!(lgc_not_equal(&Value::Compo(copied), &Value::Nil).is_err());
    }
}
