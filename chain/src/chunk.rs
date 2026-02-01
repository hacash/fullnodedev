
pub struct Chunk {
    pub height: u64,
    pub hash:   Hash,
    pub block:  Arc<dyn Block>,
    pub state:  Arc<Box<dyn State>>, // State *after* execution
    pub logs:   Arc<dyn Logs>,
    pub parent: Weak<Chunk>,
    pub childs: RwLock<Vec<Arc<Chunk>>>,
}

impl Chunk {

    pub fn update_to(&self, block: Arc<dyn Block>) -> Self {
        let mut obj = Self::new(block, self.state.clone(), self.logs.clone(), None);
        obj.childs = RwLock::new(self.children());
        obj.parent = self.parent.clone();
        obj
    }
    
    pub fn new(
        block:  Arc<dyn Block>, 
        state:  Arc<Box<dyn State>>, 
        logs:   Arc<dyn Logs>, 
        parent: Option<&Arc<Chunk>>
    ) -> Self {
        Self {
            hash: block.hash(),
            height: block.height().uint(),
            block,
            state,
            logs,
            parent: parent.map(|p| Arc::downgrade(p)).unwrap_or(Weak::new()),
            childs: RwLock::new(Vec::new()),
        }
    }

    pub fn children(&self) -> Vec<Arc<Chunk>> {
        self.childs.read().unwrap().clone()
    }

    pub fn parent(&self) -> Option<Arc<Chunk>> {
        self.parent.upgrade()
    }

    pub fn append(&self, child: Arc<Chunk>) {
        self.childs.write().unwrap().push(child);
    }


}



