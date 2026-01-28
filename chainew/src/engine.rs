



pub type FnBuildDB = fn(_: &PathBuf) -> Box<dyn DiskDB>;

pub struct ChainEngine {
    pub(crate) cnf: Arc<EngineConf>,
    pub(crate) minter: Arc<dyn Minter>,
    pub(crate) scaner: Arc<dyn Scaner>,
    pub(crate) store: Arc<BlockStore>,
    pub(crate) logs: Arc<BlockLogs>,
    pub(crate) disk: Arc<dyn DiskDB>,
    pub(crate) tree: RwLock<Roller>,
    pub(crate) rebuilding: Mutex<()>,
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
        let no_sta_dir = ! sta_dir.exists();
        if no_sta_dir {
            std::fs::create_dir_all(sta_dir).unwrap();
            std::fs::remove_dir_all(log_dir).unwrap();
            std::fs::create_dir_all(log_dir).unwrap();
        }

        let disk_db  = dbopfn(blk_dir);
        let log_db   = dbopfn(log_dir);
        let state_db = dbopfn(sta_dir);

        let disk: Arc<dyn DiskDB> = disk_db.into();
        let store = Arc::new(BlockStore::wrap(disk.clone()));
        let blogs = Arc::new(BlockLogs::wrap(log_db.into()));
        let state_db: Arc<dyn DiskDB> = state_db.into();

        let rtblk = load_root_block(minter.as_ref(), store.as_ref());
        let state = StateInst::build(state_db.clone(), None);

        let engine = ChainEngine {
            cnf: cnf.clone(),
            minter,
            scaner,
            store,
            logs: blogs.clone(),
            disk,
            tree: RwLock::new(Roller::new(
                rtblk, 
                Arc::new(Box::new(state)), 
                blogs.clone(), 
                cnf.unstable_block
            )),
            rebuilding: ().into(),
            inserting: AtomicUsize::new(0),
            recent_blocks: Mutex::new(VecDeque::new()),
            avgfees: Mutex::new(VecDeque::new()),
        };

        initialize(&engine, state_db, no_sta_dir);
        engine
    }

}


