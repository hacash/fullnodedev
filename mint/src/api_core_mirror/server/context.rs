
pub type ArcChainEngine = Arc<dyn Engine>;
pub type ArcChainNode   = Arc<dyn HNoder>;
pub type BlockCaches = Arc<Mutex<VecDeque<Arc<BlkPkg>>>>;


#[allow(unused)]
#[derive(Clone)]
pub struct ApiCtx {
    pub engine: ArcChainEngine,
    pub hcshnd: ArcChainNode,
    pub blocks: BlockCaches,
    pub miner_worker_notice_count: Arc<Mutex<u64>>,
    pub launch_time: u64,
    pub blocks_max: usize, // 4
}


impl ApiCtx {
 
    pub fn new(eng: ArcChainEngine, nd: ArcChainNode) -> ApiCtx {
        ApiCtx {
            engine: eng,
            hcshnd: nd,
            blocks: Arc::default(),
            miner_worker_notice_count: Arc::default(),
            launch_time: curtimes(),
            blocks_max: 4,
        }
    }
}