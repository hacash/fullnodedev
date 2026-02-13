

fn mei_to_hac(_: u64, buf: &[u8]) -> VmrtRes<Value> {
    let num = buf_to_uint(buf)?.to_uint();
    if num > u64::MAX as u128 {
        return itr_err_fmt!(NativeFuncError, "call mei_to_hac amount too big")
    }
    Ok(Value::Bytes( Amount::mei(num as u64).serialize() ))
}

fn hac_to_mei(_: u64, buf: &[u8]) -> VmrtRes<Value> {
    let hacash = Amount::build(buf).map_ire(NativeFuncError)?;
    let Ok(mei) = hacash.to_mei_u64() else {
        return itr_err_fmt!(NativeFuncError, "call hac_to_mei overflow")
    };
    Ok(Value::U64( mei ))
}


fn hac_to_zhu(_: u64, buf: &[u8]) -> VmrtRes<Value> {
    let hacash = Amount::build(buf).map_ire(NativeFuncError)?;
    let Ok(zhu) = hacash.to_zhu_u128() else {
        return itr_err_fmt!(NativeFuncError, "call hac_to_zhu overflow")
    };
    Ok(Value::U128( zhu ))
}


fn zhu_to_hac(_: u64, buf: &[u8]) -> VmrtRes<Value> {
    let num = buf_to_uint(buf)?.to_uint();
    if num > u64::MAX as u128 {
        return itr_err_fmt!(NativeFuncError, "call zhu_to_hac overflow")
    }
    Ok(Value::Bytes( Amount::zhu(num as u64).serialize() ))
}

fn pack_asset(_: u64, buf: &[u8]) -> VmrtRes<Value> {
    if buf.len() != 16 {
        return itr_err_fmt!(
            NativeFuncError,
            "call pack_asset expects 16 bytes (u64 + u64), got {}",
            buf.len()
        )
    }

    let serial = u64::from_be_bytes(buf[0..8].try_into().unwrap());
    let amount = u64::from_be_bytes(buf[8..16].try_into().unwrap());
    let asset = AssetAmt::from(serial, amount).map_ire(NativeFuncError)?;
    Ok(Value::Bytes(asset.serialize()))
}

fn u64_to_fold64(_: u64, buf: &[u8]) -> VmrtRes<Value> {
    let num = buf_to_uint(buf)?.to_uint();
    if num > u64::MAX as u128 {
        return itr_err_fmt!(NativeFuncError, "call u64_to_fold64 overflow")
    }
    let fold = Fold64::from(num as u64).map_ire(NativeFuncError)?;
    Ok(Value::Bytes(fold.serialize()))
}

fn fold64_to_u64(_: u64, buf: &[u8]) -> VmrtRes<Value> {
    let mut fold = Fold64::default();
    let used = fold.parse(buf).map_ire(NativeFuncError)?;
    if used != buf.len() {
        return itr_err_fmt!(
            NativeFuncError,
            "call fold64_to_u64 parse length mismatch: used {}, total {}",
            used,
            buf.len()
        )
    }
    Ok(Value::U64(fold.uint()))
}
