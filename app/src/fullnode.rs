use std::path::*;
use std::sync::*;


use super::*;

use sys::*;
use protocol::*;
use protocol::interface::*;
use protocol::component::*;



/***************************************/



struct NilDB {}
impl DiskDB for NilDB {}
struct NilScaner {}
impl Scaner for NilScaner {}

struct NilTxPool {}
impl TxPool for NilTxPool {}



/***************************************/



#[allow(dead_code)]
pub struct Builder {
    cnfini: IniObj,
    datadir: PathBuf,
    engcnf: Arc<EngineConf>,
    nodcnf: Arc<NodeConf>,
    diskdb: FnBuildDB,
    // _minter: Box<dyn Minter>,
    txpool: Arc<dyn TxPool>,
    scaner: Arc<dyn Scaner>,
    // _engine: Arc<dyn Engine>,
}

impl Builder {

    pub fn new() -> Self {
        let cnfpath = "./hacash.config.ini".to_owned();
        let cnfini = load_config(cnfpath);
        let datadir = get_mainnet_data_dir(&cnfini);
        let engcnf = Arc::new(EngineConf::new(&cnfini, HACASH_STATE_DB_UPDT));
        let nodcnf = Arc::new(NodeConf::new(&cnfini));
        let build_nil_db: FnBuildDB = |_|Box::new(NilDB{});
        Self {
            cnfini,
            datadir,
            engcnf,
            nodcnf,
            diskdb: build_nil_db,
            scaner: Arc::new(NilScaner{}),
            txpool: Arc::new(NilTxPool{}),

        }
    }

    pub fn diskdb(&mut self, f: FnBuildDB) -> &mut Self {
        self.diskdb = f;
        self
    }

    pub fn scaner(&mut self, scn: Arc<dyn Scaner>) -> &mut Self {
        self.scaner = scn;
        self
    }

    pub fn txpool(&mut self, f: fn(_: &EngineConf)->Box<dyn TxPool>) -> &mut Self {
        self.txpool = f(&self.engcnf).into();
        self
    }

    // do start all
    pub fn run(self) {
        run_fullnode(self)
    }

}





fn run_fullnode(_builder: Builder) {

    let exiter = Exiter::new();




    let worker = exiter.work();

    println!("Hello, hacash fullnode!");
    
    worker.exit();






    exiter.wait();
    println!("\n[Exit] Hacash fullnode closed.");
}







/*
// fullnode main
pub fn fullnode() {
    
    let cnfp = "./hacash.config.ini".to_string();
    let inicnf = load_config(cnfp);

    println!("[Version] full node v{}, build time: {}, database type: {}.", 
        HACASH_NODE_VERSION, HACASH_NODE_BUILD_TIME, HACASH_STATE_DB_UPDT
    );

    fullnode_with_ini(inicnf)
}


pub fn fullnode_with_ini(iniobj: IniObj) {
    let empty_scaner = Box::new(EmptyBlockScaner{});
    fullnode_with_scaner(iniobj, empty_scaner)
}


pub fn fullnode_with_scaner(iniobj: IniObj, scaner: Box<dyn Scaner>) {
    let minter = Box::new(mint::HacashMinter::create(&iniobj));
    fullnode_with_minter_scaner(iniobj, minter, scaner)
}


pub fn fullnode_with_minter_scaner(iniobj: IniObj, 
    minter: Box<dyn Minter>,
    scaner: Box<dyn Scaner>
) {

    // setup ctrl+c to quit
    let (cltx, clrx) = mpsc::channel();
    ctrlc::set_handler(move||{ let _ = cltx.send(()); }).unwrap();

    let scaner = init_block_scaner(&iniobj, Some(scaner));

    // engine
    let dbv = HACASH_STATE_DB_UPDT;
    let engine = Arc::new(ChainEngine::open(&iniobj, dbv, minter, scaner));
    
    // node & server
    let hxnode = Arc::new(HacashNode::open(&iniobj, engine.clone()));
    let hnptr = hxnode.clone();
    let server = HttpServer::open(&iniobj, engine.clone(), hnptr.clone());

    // start all
    spawn(move||{ server.start() }); // start http server
    spawn(move||{ HacashNode::start(hnptr) }); // start p2p node

    // wait to ctrl+c to exit
    clrx.recv().unwrap();
    hxnode.exit(); // wait something
    engine.exit(); // wait something

    // all exit
    println!("\n[Exit] Hacash blockchain and P2P node have been closed.");

}




// init block scaner
fn init_block_scaner(inicnf: &IniObj, blkscaner: Option<Box<dyn Scaner>>) -> Arc<dyn Scaner> {

    // scaner
    let scaner: Arc<dyn Scaner> = match blkscaner {
        Some(mut scan) => {
            scan.init(inicnf).unwrap(); // init block scaner
            scan.into()
        },
        _ => Arc::new(EmptyBlockScaner{}),
    };

    // start block scaner
    let scanercp1 = scaner.clone();
    std::thread::spawn(move||{
        scanercp1.start().unwrap();
    });
    let scanercp2 = scaner.clone();
    std::thread::spawn(move||{
        scanercp2.serve().unwrap();
    });
    
    // ok
    scaner
}

*/
