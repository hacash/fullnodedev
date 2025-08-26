
use app::*;
use app::fullnode::Builder;
use sys::{never, IniObj};
use protocol::interface::*;
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

    // build & setup
    let mut builder =  Builder::new();
    builder.diskdb(|dir|Box::new(db::DiskKV::open(dir)));
    builder.txpool(build_txpool);

    // start run
    builder.run();
}


fn build_txpool(_ini: &IniObj) -> Box<dyn TxPool> {
    


    unimplemented!()
}
