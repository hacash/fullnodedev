



pub type FnBuildDB = fn(_: &PathBuf) -> Box<dyn DiskDB>;

pub struct ChainEngine {
    pub(crate) cnf: Arc<EngineConf>,
    pub(crate) minter: Arc<dyn Minter>,
    pub(crate) scaner: Arc<dyn Scaner>,
    pub(crate) store: Arc<BlockStore>,
    pub(crate) logs: Arc<BlockLogs>,
    pub(crate) disk: Arc<dyn DiskDB>,
    pub(crate) tree: RwLock<Roller>,
    pub(crate) isrtlk: Mutex<()>,
    pub(crate) inserting: AtomicUsize,
    // Caches
    pub(crate) recent_blocks: Mutex<VecDeque<Arc<RecentBlockInfo>>>,
    pub(crate) avgfees: Mutex<VecDeque<u64>>,
}



impl ChainEngine {

    pub fn open(
        dbopfn: FnBuildDB,
        cnf: Arc<EngineConf>,
        minter: Arc<dyn Minter>,
        scaner: Arc<dyn Scaner>
    ) -> ChainEngine {
        let blk_dir = &cnf.block_data_dir;
        let sta_dir = &cnf.state_data_dir;
        let log_dir = &cnf.blogs_data_dir;
        
        std::fs::create_dir_all(blk_dir).unwrap();
        std::fs::create_dir_all(log_dir).unwrap();
        let state_exists = sta_dir.exists();
        if !state_exists {
            std::fs::create_dir_all(sta_dir).unwrap();
        }

        let disk_db = dbopfn(blk_dir);
        let log_db = dbopfn(log_dir);
        let state_db = dbopfn(sta_dir);

        let disk: Arc<dyn DiskDB> = disk_db.into();
        let store = Arc::new(BlockStore::wrap(disk.clone()));
        let logs = Arc::new(BlockLogs::wrap(log_db.into()));
        let state_db: Arc<dyn DiskDB> = state_db.into();

        let gen_blk = minter.genesis_block();
        let gen_state = StateInst::build(state_db.clone(), None);

        let engine = ChainEngine {
            cnf: cnf.clone(),
            minter,
            scaner,
            store,
            logs: logs.clone(),
            disk,
            tree: RwLock::new(Roller::new(
                gen_blk, 
                Arc::new(Box::new(gen_state)), 
                logs.clone(), 
                cnf.unstable_block
            )),
            isrtlk: Mutex::new(()),
            inserting: AtomicUsize::new(0),
            recent_blocks: Mutex::new(VecDeque::new()),
            avgfees: Mutex::new(VecDeque::new()),
        };

        initialize(&engine, state_db, state_exists);
        engine
    }

}


