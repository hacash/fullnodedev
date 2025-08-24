

const H32: usize = 32;

// height, intro bytes
pub type FnExtendBlockHashFunc = fn(_: u64, _: &[u8]) -> [u8; H32];


fn default_block_hash(_: u64, stuff: &[u8]) -> [u8; H32] {
    sys::calculate_hash(stuff)
}


pub static mut EXTEND_BLOCK_HASH_FUNC: FnExtendBlockHashFunc = default_block_hash;


pub fn setup_block_hash(f: FnExtendBlockHashFunc) {
    unsafe {
        EXTEND_BLOCK_HASH_FUNC = f;
    }
}



