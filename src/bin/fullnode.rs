// use basis::server::ApiCtx;
use app::fullnode::Builder;
use app::{fullnode::NilScaner, *};
use basis::config::*;
use basis::interface::*;
use chain::*;
use mint::HacashMinter;
use node::{memtxpool::*, node::HacashNode};
use server::*;
use sys::*;

/*

* fullnode main
*/
#[allow(dead_code)]
fn main() {
    println!(
        "[Version] full node v{}, build time: {}, database type: {}.",
        HACASH_NODE_VERSION, HACASH_NODE_BUILD_TIME, HACASH_STATE_DB_UPDT
    );

    run();
}

pub fn run() {
    run_with_scaner("./hacash.config.ini", Box::new(NilScaner {}));
}

pub fn run_with_config(cnfpath: &str) {
    run_with_scaner(cnfpath, Box::new(NilScaner {}));
}

pub fn run_with_scaner(cnfpath: &str, scan: Box<dyn Scaner>) {
    // setup
    protocol::setup::block_hasher(x16rs::block_hash);
    protocol::setup::action_register(
        protocol::action::try_create,
        protocol::action::try_json_decode,
    );
    protocol::setup::action_register(mint::action::try_create, mint::action::try_json_decode);
    // let mut server_apis: Vec<Router<ApiCtx>> = vec![];

    // tex feature
    #[cfg(feature = "tex")]
    {
        protocol::setup::action_register(protocol::tex::try_create, protocol::tex::try_json_decode);
    }
    // vm feature (contracts + p2sh)
    #[cfg(feature = "vm")]
    {
        protocol::setup::action_register(vm::action::try_create, vm::action::try_json_decode);
        protocol::setup::action_hooker(vm::hook::try_action_hook);
        protocol::setup::vm_assigner(vm::machine::vm_assign);
    }

    // api
    // build & setup
    server::setup::api_servicer(scan.api_services());
    let mut builder = Builder::new(cnfpath);

    // Configure global VM contract cache pool (performance-only).
    #[cfg(feature = "vm")]
    {
        let engcnf = builder.engine_conf();
        let size_mb = engcnf.contract_cache_size;
        let bytes = if size_mb.is_finite() && size_mb > 0.0 {
            (size_mb * 1024.0 * 1024.0) as usize
        } else {
            0
        };
        vm::configure_contract_cache(vm::machine::ContractCacheConfig {
            max_bytes: bytes,
            ..Default::default()
        });
    }

    builder
        .diskdb(|dir| Box::new(db::DiskKV::open(dir)))
        .scaner(scan)
        .txpool(build_txpool)
        .minter(|ini| Box::new(HacashMinter::create(ini)))
        .engine(|dbfn, cnf, minter, scaner| Box::new(ChainEngine::open(dbfn, cnf, minter, scaner)))
        .hnoder(|ini, txpool, engine| Box::new(HacashNode::open(ini, txpool, engine)))
        .server(|ini, hnoder| {
            #[allow(unused_mut)]
            let mut services: Vec<std::sync::Arc<dyn ApiService>> =
                vec![mint::api_service::service()];
            #[cfg(feature = "vm")]
            services.push(vm::api_service::service());
            Box::new(HttpServer::open(
                ini,
                hnoder.clone(),
                server::router(hnoder, vec![], services),
            ))
        })
        .app(diabider::start_diamond_auto_bidding);

    // start run
    builder.run();
}

fn build_txpool(engcnf: &EngineConf) -> Box<dyn TxPool> {
    let mut tpmaxs = maybe!(
        engcnf.miner_enable,
        vec![2000, 100], // miner node
        vec![10, 10]     // normal node
    );
    let fpmds = vec![true, false]; // is sort by fee_purity, normal or diamint
    cover(&mut tpmaxs, &engcnf.txpool_maxs);
    Box::new(MemTxPool::new(engcnf.lowest_fee_purity, tpmaxs, fpmds))
}
