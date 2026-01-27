
pub struct Chunk {
    pub height: u64,
    pub hash: Hash,
    pub block: Arc<dyn Block>,
    pub state: Arc<Box<dyn State>>, // State *after* execution
    pub logs:  Arc<dyn Logs>,
    pub parent: Weak<Chunk>,
    pub children: RwLock<Vec<Arc<Chunk>>>,
}

impl Chunk {
    
    pub fn new(block: Arc<dyn Block>, state: Arc<Box<dyn State>>, logs: Arc<dyn Logs>, parent: Option<&Arc<Chunk>>) -> Self {
        Self {
            hash: block.hash(),
            height: block.height().uint(),
            block,
            state,
            logs,
            parent: parent.map(|p| Arc::downgrade(p)).unwrap_or(Weak::new()),
            children: RwLock::new(Vec::new()),
        }
    }

    pub fn parent(&self) -> Option<Arc<Chunk>> {
        self.parent.upgrade()
    }

    pub fn append(&self, child: Arc<Chunk>) {
        self.children.write().unwrap().push(child);
    }


}



