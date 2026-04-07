const H32: usize = 32;

// height, intro bytes
pub type FnBlockHasherFunc = fn(_: u64, _: &[u8]) -> [u8; H32];

pub fn default_block_hasher(_: u64, stuff: &[u8]) -> [u8; H32] {
    calculate_hash(stuff)
}

pub fn do_block_hash(height: u64, stuff: &[u8]) -> [u8; H32] {
    match current_setup() {
        Ok(registry) => (registry.block_hasher)(height, stuff),
        Err(e) => panic!("protocol setup missing: {}", e),
    }
}
