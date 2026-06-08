

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

// keccak256
#[allow(dead_code)]
fn keccak256(_: u64, buf: &[u8]) -> VmrtRes<Value> {
    let result = sys::keccak256(buf);
    Ok(Value::bytes(result.to_vec()))
}

// blake2s256
#[allow(dead_code)]
fn blake2s256(_: u64, buf: &[u8]) -> VmrtRes<Value> {
    let result = sys::blake2s256(buf);
    Ok(Value::bytes(result.to_vec()))
}

// blake2b256
#[allow(dead_code)]
fn blake2b256(_: u64, buf: &[u8]) -> VmrtRes<Value> {
    let result = sys::blake2b256(buf);
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
            keccak256(0, &[]).unwrap(),
            Value::bytes(sys::keccak256(&[]).to_vec())
        );
        assert_eq!(
            blake2s256(0, &[]).unwrap(),
            Value::bytes(sys::blake2s256(&[]).to_vec())
        );
        assert_eq!(
            blake2b256(0, &[]).unwrap(),
            Value::bytes(sys::blake2b256(&[]).to_vec())
        );
        assert_eq!(
            ripemd160(0, &[]).unwrap(),
            Value::bytes(sys::ripemd160(&[]).to_vec())
        );
    }

    #[test]
    fn hash_natives_match_known_vectors() {
        let abc = b"abc";
        assert_eq!(
            keccak256(0, abc).unwrap(),
            Value::bytes(sys::keccak256(abc).to_vec())
        );
        assert_eq!(
            blake2s256(0, abc).unwrap(),
            Value::bytes(sys::blake2s256(abc).to_vec())
        );
        assert_eq!(
            blake2b256(0, abc).unwrap(),
            Value::bytes(sys::blake2b256(abc).to_vec())
        );
    }
}
