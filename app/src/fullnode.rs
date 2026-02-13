use std::path::*;
use std::sync::*;
use std::thread::*;

use super::*;

use basis::config::*;
use basis::interface::*;
use sys::*;

/***************************************/

pub type FnDBCreater = fn(_: &PathBuf) -> Box<dyn DiskDB>;
pub type FnExtendApp = fn(_: Worker, _: Arc<dyn HNoder>);

/***************************************/

struct NilKVDB {}
impl DiskDB for NilKVDB {}

pub struct NilScaner {}
impl Scaner for NilScaner {}

struct NilTxPool {}
impl TxPool for NilTxPool {}

struct NilMinter {}
impl Minter for NilMinter {}

struct NilEngine {}
impl EngineRead for NilEngine {}
impl Engine for NilEngine {}

struct NilHNoder {}
impl HNoder for NilHNoder {}

struct NilServer {}
impl Server for NilServer {}

/***************************************/

#[allow(dead_code)]
pub struct Builder {
    pub exiter: Exiter,
    cnfini: IniObj,
    datdir: PathBuf,
    engcnf: Arc<EngineConf>,
    nodcnf: Arc<NodeConf>,
    diskdb: FnDBCreater,
    extapp: Vec<FnExtendApp>,
    minter: Arc<dyn Minter>,
    engine: Arc<dyn Engine>,
    txpool: Arc<dyn TxPool>,
    scaner: Arc<dyn Scaner>,
    hnoder: Arc<dyn HNoder>,
    server: Arc<dyn Server>,
}

impl Builder {
    pub fn new(inipath: &str) -> Self {
        let cnfpath = inipath.to_owned(); // "./hacash.config.ini".to_owned();
        let cnfini = load_config(cnfpath);
        let datdir = get_mainnet_data_dir(&cnfini);
        let engcnf = Arc::new(EngineConf::new(&cnfini, HACASH_STATE_DB_UPDT));
        let nodcnf = Arc::new(NodeConf::new(&cnfini));
        let build_nil_db: FnDBCreater = |_| Box::new(NilKVDB {});
        let exiter = Exiter::new();
        let exitdo = exiter.clone();
        let _ = ctrlc::set_handler(move || {
            exitdo.exit();
        });
        Self {
            exiter,
            cnfini,
            datdir,
            engcnf,
            nodcnf,
            diskdb: build_nil_db,
            extapp: Vec::new(),
            scaner: Arc::new(NilScaner {}),
            txpool: Arc::new(NilTxPool {}),
            minter: Arc::new(NilMinter {}),
            engine: Arc::new(NilEngine {}),
            hnoder: Arc::new(NilHNoder {}),
            server: Arc::new(NilServer {}),
        }
    }

    pub fn diskdb(&mut self, f: FnDBCreater) -> &mut Self {
        self.diskdb = f;
        self
    }

    pub fn engine_conf(&self) -> Arc<EngineConf> {
        self.engcnf.clone()
    }

    pub fn txpool(&mut self, f: fn(_: &EngineConf) -> Box<dyn TxPool>) -> &mut Self {
        self.txpool = f(&self.engcnf).into();
        self
    }

    pub fn minter(&mut self, f: fn(_: &IniObj) -> Box<dyn Minter>) -> &mut Self {
        self.minter = f(&self.cnfini).into();
        self
    }

    pub fn scaner(&mut self, scn: Box<dyn Scaner>) -> &mut Self {
        self.scaner = scn.into();
        self
    }

    pub fn engine(
        &mut self,
        f: fn(
            _: FnDBCreater,
            _: Arc<EngineConf>,
            _: Arc<dyn Minter>,
            _: Arc<dyn Scaner>,
        ) -> Box<dyn Engine>,
    ) -> &mut Self {
        self.engine = f(
            self.diskdb,
            self.engcnf.clone(),
            self.minter.clone(),
            self.scaner.clone(),
        )
        .into();
        self
    }

    pub fn hnoder(
        &mut self,
        f: fn(_: &IniObj, _: Arc<dyn TxPool>, _: Arc<dyn Engine>) -> Box<dyn HNoder>,
    ) -> &mut Self {
        self.hnoder = f(&self.cnfini, self.txpool.clone(), self.engine.clone()).into();
        self
    }

    pub fn server(
        &mut self,
        f: fn(_: &IniObj, _: Arc<dyn HNoder>) -> Box<dyn Server>,
    ) -> &mut Self {
        self.server = f(&self.cnfini, self.hnoder.clone()).into();
        self
    }

    pub fn app(&mut self, f: FnExtendApp) -> &mut Self {
        self.extapp.push(f);
        self
    }

    // do start all
    pub fn run(self) {
        run_fullnode(self)
    }
}

fn run_fullnode(builder: Builder) {
    let exiter = builder.exiter.clone();

    // unpack
    // let _txpool = builder.txpool.clone();
    // let _scaner = builder.scaner.clone();
    // let _minter = builder.minter.clone();
    // let _engine = builder.engine.clone();
    let server = builder.server.clone();
    let scanr1 = builder.scaner.clone();
    let scanr2 = builder.scaner.clone();
    let hnoder = builder.hnoder.clone();
    let hnode2 = builder.hnoder.clone();

    // start server
    let wkr1 = exiter.worker();
    spawn(move || server.start(wkr1));

    // start scaner
    let wkr2 = exiter.worker();
    spawn(move || scanr1.start(wkr2));
    let wkr3 = exiter.worker();
    spawn(move || scanr2.serve(wkr3));

    // start node
    let wkr4 = exiter.worker();
    spawn(move || hnoder.start(wkr4));

    // run extend app
    for app in builder.extapp {
        let wkr = exiter.worker();
        let hnoder = builder.hnoder.clone();
        spawn(move || app(wkr, hnoder));
    }
    // start all and wait them exit
    exiter.wait();
    hnode2.exit();
    println!("[Exit] Hacash fullnode closed.");
}
