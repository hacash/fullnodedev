
use mint::HacashMinter;
use sys::*;
use app::*;
use app::fullnode::Builder;
use protocol::{interface::*, EngineConf};
use chain::engine::*;
use node::memtxpool::*;


/*

* fullnode main
*/ 
#[allow(dead_code)]
fn main() {

    println!("[Version] full node v{}, build time: {}, database type: {}.", 
        HACASH_NODE_VERSION, HACASH_NODE_BUILD_TIME, HACASH_STATE_DB_UPDT
    );

    // build & setup
    let mut builder =  Builder::new();

    builder.diskdb(|dir|Box::new(db::DiskKV::open(dir)))
        .minter(|ini|Box::new(HacashMinter::create(ini)))
        .engine(|dbfn, cnf, minter, scaner|Box::new(ChainEngine::open(dbfn, cnf, minter, scaner)))
        .txpool(build_txpool)
        ;

    // start run
    builder.run();
}


fn build_txpool(engcnf: &EngineConf) -> Box<dyn TxPool> {
    let mut tpmaxs = maybe!(engcnf.miner_enable,
        vec![2000, 100], // miner node
        vec![10, 10]     // normal node
    );
    let fpmds  = vec![true, false]; // is sort by fee_purity, normal or diamint
    cover(&mut tpmaxs, &engcnf.txpool_maxs);
    Box::new(MemTxPool::new(
        engcnf.lowest_fee_purity, 
        tpmaxs, 
        fpmds
    ))
}
