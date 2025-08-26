


use app::*;

/*
* fullnode main
*/ 
#[allow(dead_code)]
fn main() {

    println!("[Version] full node v{}, build time: {}, database type: {}.", 
        HACASH_NODE_VERSION, HACASH_NODE_BUILD_TIME, HACASH_STATE_DB_UPDT
    );

    // setup hook
    protocol::block::setup_block_hasher( x16rs::block_hash );

    // build and start
    let mut builder = app::fullnode::Builder::new();

    // open and setup kv database
    let dkv = Box::new(db::DiskKV::open(builder.datadir()));
    builder.diskkv(dkv);

    
    builder.run();

}



