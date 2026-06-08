use blake2::digest::consts::U32;
use blake2::{Blake2b, Blake2s256, Digest as BlakeDigest};
use sha2::Sha256;
use sha3::{Keccak256, Sha3_256};
use ripemd::Ripemd160;

pub const H32S: usize = 32;



pub fn calculate_hash(data: impl AsRef<[u8]>) -> [u8; H32S] {
    sha3(data)
}

// sha3-256
pub fn sha3(data: impl AsRef<[u8]>) -> [u8; H32S] {
    let mut hasher = Sha3_256::new();
    hasher.update(data);
    let result = hasher.finalize();
    let result: [u8; H32S] = result[..].try_into().unwrap();
    result
}


// sha2-256
pub fn sha2(data: impl AsRef<[u8]>) -> [u8; H32S] {
    let mut hasher = Sha256::new();
    hasher.update(data);
    let result = hasher.finalize();
    let result: [u8; H32S] = result[..].try_into().unwrap();
    result
}


// keccak-256
pub fn keccak256(data: impl AsRef<[u8]>) -> [u8; H32S] {
    let mut hasher = Keccak256::new();
    hasher.update(data);
    let result = hasher.finalize();
    let result: [u8; H32S] = result[..].try_into().unwrap();
    result
}


// blake2s-256
pub fn blake2s256(data: impl AsRef<[u8]>) -> [u8; H32S] {
    let mut hasher = Blake2s256::new();
    hasher.update(data);
    let result = hasher.finalize();
    let result: [u8; H32S] = result[..].try_into().unwrap();
    result
}

// blake2b-256
pub fn blake2b256(data: impl AsRef<[u8]>) -> [u8; H32S] {
    let mut hasher = Blake2b::<U32>::new();
    hasher.update(data);
    let result = hasher.finalize();
    let result: [u8; H32S] = result[..].try_into().unwrap();
    result
}



pub fn ripemd160(data: impl AsRef<[u8]>) -> [u8; 20] {
    let mut hasher = Ripemd160::new();
    hasher.update(data);
    // to [u8; 20]
    let result = hasher.finalize();
    let result: [u8; 20] = result[..].try_into().unwrap();
    result
}

#[cfg(test)]
mod hash_tests {
    use super::*;

    fn hex(bytes: &[u8]) -> String {
        bytes.iter().map(|b| format!("{:02x}", b)).collect()
    }

    #[test]
    fn keccak256_matches_known_vectors() {
        assert_eq!(
            hex(&keccak256([])),
            "c5d2460186f7233c927e7db2dcc703c0e500b653ca82273b7bfad8045d85a470"
        );
        assert_eq!(
            hex(&keccak256(b"abc")),
            "4e03657aea45a94fc7d47ba826c8d667c0d1e6e33a64a036ec44f58fa12d6c45"
        );
        assert_eq!(
            hex(&keccak256(b"The quick brown fox jumps over the lazy dog")),
            "4d741b6f1eb29cb2a9b9911c82f56fa8d73b04959d3d9d222895df6c0b28aa15"
        );
    }

    #[test]
    fn blake2s256_matches_known_vectors() {
        assert_eq!(
            hex(&blake2s256([])),
            "69217a3079908094e11121d042354a7c1f55b6482ca1a51e1b250dfd1ed0eef9"
        );
        assert_eq!(
            hex(&blake2s256(b"abc")),
            "508c5e8c327c14e2e1a72ba34eeb452f37458b209ed63a294d999b4c86675982"
        );
        assert_eq!(
            hex(&blake2s256(b"The quick brown fox jumps over the lazy dog")),
            "606beeec743ccbeff6cbcdf5d5302aa855c256c29b88c8ed331ea1a6bf3c8812"
        );
    }

    #[test]
    fn blake2b256_matches_known_vectors() {
        assert_eq!(
            hex(&blake2b256([])),
            "0e5751c026e543b2e8ab2eb06099daa1d1e5df47778f7787faab45cdf12fe3a8"
        );
        assert_eq!(
            hex(&blake2b256(b"abc")),
            "bddd813c634239723171ef3fee98579b94964e3bb1cb3e427262c8c068d52319"
        );
        assert_eq!(
            hex(&blake2b256(b"The quick brown fox jumps over the lazy dog")),
            "01718cec35cd3d796dd00020e0bfecb473ad23457d063b75eff29c0ffa2e58a9"
        );
    }

    #[test]
    fn hash_algorithms_diverge_on_same_input() {
        let input = b"same-input";
        assert_ne!(sha2(input), sha3(input));
        assert_ne!(sha2(input), keccak256(input));
        assert_ne!(sha2(input), blake2s256(input));
        assert_ne!(sha2(input), blake2b256(input));
        assert_ne!(sha3(input), keccak256(input));
        assert_ne!(sha3(input), blake2s256(input));
        assert_ne!(sha3(input), blake2b256(input));
        assert_ne!(keccak256(input), blake2s256(input));
        assert_ne!(keccak256(input), blake2b256(input));
        assert_ne!(blake2s256(input), blake2b256(input));
    }
}
