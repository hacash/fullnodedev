use super::super::*;
use super::ChunkRef;

pub(crate) struct Roller {
    level: u64,
    root: ChunkRef,
    head: ChunkRef,
    // pub tree: HashMap<Hash, ChunkRef>,
}

impl Roller {
    pub(crate) fn new(
        root_blk: Arc<dyn Block>,
        root_sta: Arc<Box<dyn State>>,
        root_log: Arc<dyn Logs>,
        level: u64,
    ) -> Self {
        let root = ChunkRef::new(root_blk, root_sta, root_log, None);
        Self {
            level,
            head: root.clone(),
            root,
        }
    }

    pub(crate) fn root(&self) -> ChunkRef {
        self.root.clone()
    }

    pub(crate) fn head(&self) -> ChunkRef {
        self.head.clone()
    }

    pub(crate) fn root_height(&self) -> u64 {
        self.root.height()
    }

    pub(crate) fn head_height(&self) -> u64 {
        self.head.height()
    }

    pub(crate) fn reset_root_head(&mut self, node: ChunkRef) {
        self.root = node.clone();
        self.head = node;
    }

    pub(crate) fn quick_find(&self, hash: &Hash) -> Option<ChunkRef> {
        if self.head.hash() == hash {
            return Some(self.head.clone());
        }
        let mut stack = vec![self.root.clone()];
        while let Some(node) = stack.pop() {
            if node.hash() == hash {
                return Some(node);
            }
            for child in node.children() {
                stack.push(child);
            }
        }
        None
    }

    pub(crate) fn has_child_hash(&self, parent: &ChunkRef, hash: &Hash) -> bool {
        parent.children().iter().any(|child| child.hash() == hash)
    }

    pub(crate) fn insert_child(
        &mut self,
        parent: &ChunkRef,
        block: Arc<dyn Block>,
        state: Arc<Box<dyn State>>,
        logs: Arc<dyn Logs>,
        fast_sync: bool,
    ) -> Ret<(Option<ChunkRef>, Option<ChunkRef>)> {
        let child = ChunkRef::new(block, state, logs, Some(parent));
        self.insert(parent, child, fast_sync)
    }

    pub(crate) fn collect_back_hashes(from: &ChunkRef, max_num: u64) -> Vec<(BlockHeight, Hash)> {
        let mut vec = Vec::new();
        let mut seek = from.clone();
        let mut skhei = BlockHeight::from(seek.height());
        for _ in 0..max_num {
            vec.push((skhei.clone(), seek.hash().clone()));
            let Some(parent) = seek.parent() else { break };
            seek = parent;
            skhei -= 1;
        }
        vec
    }

    fn insert(
        &mut self,
        parent: &ChunkRef,
        child: ChunkRef,
        fast_sync: bool,
    ) -> Ret<(Option<ChunkRef>, Option<ChunkRef>)> {
        parent.append(&child, fast_sync)?;
        let mut head_change: Option<ChunkRef> = None;
        let mut root_change: Option<ChunkRef> = None;
        if child.height() > self.head.height() {
            self.head = child.clone();
            head_change = Some(self.head.clone());
            if self.head.height() > self.root.height() + self.level {
                let new_root_height = self.root.height() + 1;
                let Some(new_root) = Self::trace_parent(self.head.clone(), new_root_height) else {
                    return errf!("root height {} not found when tracing", new_root_height);
                };
                self.root = new_root.clone();
                root_change = Some(new_root);
            }
        }
        Ok((root_change, head_change))
    }

    fn trace_parent(mut seek: ChunkRef, parent_hei: u64) -> Option<ChunkRef> {
        if parent_hei > seek.height() {
            return None;
        }
        while seek.height() != parent_hei {
            if let Some(sk) = seek.parent() {
                seek = sk;
            } else {
                return None;
            }
        }
        Some(seek)
    }
}
