
// use basis::server::ApiCtx;
use basis::interface::*;
use basis::config::*;
use sys::*;
use app::{fullnode::NilScaner, *};
use app::fullnode::Builder;
use chain::engine::*;
use node::{memtxpool::*, node::HacashNode};
use mint::HacashMinter;
use server::*;

/*

* fullnode main
*/ 
#[allow(dead_code)]
fn main() {

    println!("[Version] full node v{}, build time: {}, database type: {}.", 
        HACASH_NODE_VERSION, HACASH_NODE_BUILD_TIME, HACASH_STATE_DB_UPDT
    );

    run();

}


pub fn run() {
    run_with_scaner("./hacash.config.ini", Box::new(NilScaner{}));
}


pub fn run_with_config(cnfpath: &str) {
    run_with_scaner(cnfpath, Box::new(NilScaner{}));
}


pub fn run_with_scaner(cnfpath: &str, scan: Box<dyn Scaner>) {

    // setup 
    protocol::setup::block_hasher( x16rs::block_hash );
    protocol::setup::action_register( protocol::action::try_create );
    protocol::setup::action_register( mint::action::try_create );
    // let mut server_apis: Vec<Router<ApiCtx>> = vec![];

    // tex feature
    #[cfg(feature = "tex")]
    {
        protocol::setup::action_register( protocol::tex::try_create );
    }
    // hvm feature
    #[cfg(feature = "hvm")]
    {
        protocol::setup::action_register( vm::action::try_create );
        protocol::setup::action_hooker( vm::hook::try_action_hook );
    }

    // api 
    // build & setup
    let mut builder =  Builder::new(cnfpath);

    builder.diskdb(|dir|Box::new(db::DiskKV::open(dir)))
        .scaner(scan)
        .txpool(build_txpool)
        .minter(|ini|Box::new(HacashMinter::create(ini)))
        .engine(|dbfn, cnf, minter, scaner|Box::new(ChainEngine::open(dbfn, cnf, minter, scaner)))
        .hnoder(|ini, txpool, engine|Box::new(HacashNode::open(ini, txpool, engine)))
        .server(|ini, hnoder|Box::new(HttpServer::open(ini, hnoder.clone(), server::router(hnoder, vec![]))))
        .app(diabider::start_diamond_auto_bidding)
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
