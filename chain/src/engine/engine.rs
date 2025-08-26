
// block inserting status
const ISRT_STAT_IDLE:     usize = 0;
const ISRT_STAT_DISCOVER: usize = 1;
const ISRT_STAT_SYNCING:  usize = 2;


// (new root, new head, blk data) 
// type InsertResult = Ret<(Option<Arc<Chunk>>, Option<Arc<Chunk>>, Vec<u8>)>;



#[allow(dead_code)]
pub struct ChainEngine {
    cnf: Arc<EngineConf>,
    // 
    minter: Arc<dyn Minter>,
    scaner: Arc<dyn Scaner>,

    // data
    disk: Arc<dyn DiskDB>,
    store: BlockStore,

    roller: Mutex<Roller>,

    isrtlk: Mutex<()>,
    inserting: AtomicUsize, // 0:not  1:discover  2:sync

    // data cache
    rctblks: Mutex<VecDeque<Arc<RecentBlockInfo>>>,
    avgfees: Mutex<VecDeque<u64>>,

}


impl ChainEngine {


    pub fn open(
        dbopfn: FnBuildDB,
        cnf: Arc<EngineConf>,
        minter: Arc<dyn Minter>,
        scaner: Arc<dyn Scaner>
    ) -> ChainEngine {
        // cnf
        let blk_dir = &cnf.block_data_dir;
        let sta_dir = &cnf.state_data_dir;
        let is_state_upgrade = !sta_dir.exists(); // not find new dir
        std::fs::create_dir_all(blk_dir).unwrap();
        std::fs::create_dir_all(sta_dir).unwrap();
        // build
        let disk: Arc<dyn DiskDB> = dbopfn(blk_dir).into();
        // if state database upgrade
        let sta_db = dbopfn(sta_dir);
        dev_count_switch_print(cnf.dev_count_switch, sta_db.as_ref()); // dev test
        let state = StateInst::build(sta_db.into(), Weak::<StateInst>::new());
        let staptr = Arc::new(state);
        // base or genesis block
        let bsblk =  load_root_block(minter.as_ref(), disk.clone(), is_state_upgrade);
        let roller = Roller::create(cnf.unstable_block, bsblk, staptr);
        let roller = Mutex::new(roller);
        // engine
        let d1 = disk.clone();
        let mut engine = ChainEngine {
            cnf,
            minter,
            scaner,
            roller,
            disk,
            rctblks: Mutex::default(),
            avgfees: Mutex::default(),
            store: BlockStore::wrap(d1),
            isrtlk: ().into(),
            inserting: AtomicUsize::new(0),
        };
        rebuild_unstable_blocks(&mut engine);
        engine
    }



}
