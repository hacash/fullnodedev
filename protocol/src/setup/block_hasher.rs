

const H32: usize = 32;

// height, intro bytes
pub type FnBlockHasherFunc = fn(_: u64, _: &[u8]) -> [u8; H32];


fn default_block_hasher(_: u64, stuff: &[u8]) -> [u8; H32] {
    sys::calculate_hash(stuff)
}


pub static mut BLOCK_HASHER_FUNC: FnBlockHasherFunc = default_block_hasher;


pub fn block_hasher(f: FnBlockHasherFunc) {
    unsafe {
        BLOCK_HASHER_FUNC = f;
    }
}



