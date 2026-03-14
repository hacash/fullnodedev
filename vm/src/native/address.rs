
fn address_ptr(_: u64, buf: &[u8]) -> VmrtRes<Value> {
    if buf.len() != 1 {
        return itr_err_fmt!(NativeFuncError, "param error")
    }
    const DVN: u8 = ADDR_OR_PTR_DIV_NUM;
    let idx = buf[0];
    let max = u8::MAX - DVN;
    if idx > max {
        return itr_err_fmt!(NativeFuncError, "address_ptr param max {} but got {}", max, idx)
    }
    Ok(Value::U8(idx + DVN))
}

// context_address is handled directly by the interpreter (NTENV dispatch)
// without serialize/deserialize roundtrip.



