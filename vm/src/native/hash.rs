

// sha3
fn sha3(_: u64, buf: &[u8]) -> VmrtRes<Value> {
    let result = sys::sha3(buf);
    Ok(Value::bytes(result.to_vec()))
}


// sha2
#[allow(dead_code)]
fn sha2(_: u64, buf: &[u8]) -> VmrtRes<Value> {
    let result = sys::sha2(buf);
    Ok(Value::bytes(result.to_vec()))
}


// ripemd160
#[allow(dead_code)]
fn ripemd160(_: u64, buf: &[u8]) -> VmrtRes<Value> {
    let result = sys::ripemd160(buf);
    Ok(Value::bytes(result.to_vec()))
}

#[cfg(test)]
mod hash_native_tests {
    use super::*;

    #[test]
    fn hash_natives_accept_empty_input() {
        assert_eq!(sha2(0, &[]).unwrap(), Value::bytes(sys::sha2(&[]).to_vec()));
        assert_eq!(sha3(0, &[]).unwrap(), Value::bytes(sys::sha3(&[]).to_vec()));
        assert_eq!(
            ripemd160(0, &[]).unwrap(),
            Value::bytes(sys::ripemd160(&[]).to_vec())
        );
    }
}
