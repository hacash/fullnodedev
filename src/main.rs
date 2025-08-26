fn main() {

    protocol::block::setup_block_hasher( x16rs::block_hash );

    use protocol::interface::*;
    let _memkvinst = db::MemKV::new();

    println!("Hello, world!");
}
