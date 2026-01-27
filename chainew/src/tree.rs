
pub struct Roller {
    pub level: u64,
    pub root: Arc<Chunk>,
    pub head: Arc<Chunk>,
    pub tree: HashMap<Hash, Arc<Chunk>>,
}

impl Roller {

    pub fn new(
        root_blk: Arc<dyn Block>, 
        root_sta: Arc<Box<dyn State>>, 
        root_log: Arc<dyn Logs>, 
        level: u64
    ) -> Self {
        let root = Arc::new(Chunk::new(root_blk, root_sta, root_log, None));
        let mut tree = HashMap::new();
        tree.insert(root.hash.clone(), root.clone());
        Self {
            level,
            head: root.clone(),
            root,
            tree,
        }
    }

    pub fn insert(&mut self, parent: &Arc<Chunk>, child: Arc<Chunk>) -> (Option<Arc<Chunk>>, Option<Arc<Chunk>>) {
        self.tree.insert(child.hash.clone(), child.clone());
        parent.children.write().unwrap().push(child.clone());
        let mut head_change: Option<Arc<Chunk>> = None;
        let mut root_change: Option<Arc<Chunk>> = None;
        // if change head
        if child.height > self.head.height {
            self.head = child.clone();
            head_change = Some(self.head.clone());
            // if change root
            if self.head.height > self.root.height + self.level {
                let new_root_height = self.root.height + 1;
                let Some(new_root) = Self::trace_parent(self.head.clone(), new_root_height) else {
                    panic!("cannot trace root height {}", new_root_height)
                };
                self.root = new_root.clone();
                self.rebuild_index_from_root();
                root_change = Some(new_root);
            }
        }
        (root_change, head_change)
    }

    pub fn quick_find(&self, hash: &Hash) -> Option<Arc<Chunk>> {
        if self.head.hash == *hash {
            return Some(self.head.clone())
        }
        self.tree.get(hash).cloned()
    }


}


impl Roller {

    fn rebuild_index_from_root(&mut self) {
        let mut ntree = HashMap::new();
        let mut stack = vec![self.root.clone()];
        while let Some(node) = stack.pop() {
            ntree.insert(node.hash.clone(), node.clone());
            let children = node.children.read().unwrap();
            for child in children.iter() {
                stack.push(child.clone());
            }
        }
        self.tree = ntree;
    }


    fn trace_parent(mut seek: Arc<Chunk>, parent_hei: u64) -> Option<Arc<Chunk>> {
        if parent_hei > seek.height {
            return None
        }
        while seek.height != parent_hei {
            if let Some(sk) = seek.parent.upgrade() {
                seek = sk;
            } else {
                return None
            }
        }
        Some(seek)
    }

}
