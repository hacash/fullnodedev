use super::super::*;

struct Chunk {
    height: u64,
    hash: Hash,
    block: Arc<dyn Block>,
    state: Arc<Box<dyn State>>, // State *after* execution
    logs: Arc<dyn Logs>,
    parent: Weak<Chunk>,
    childs: RwLock<Vec<Arc<Chunk>>>,
}

#[derive(Clone)]
pub(crate) struct ChunkRef(Arc<Chunk>);

impl Chunk {
    fn new(
        block: Arc<dyn Block>,
        state: Arc<Box<dyn State>>,
        logs: Arc<dyn Logs>,
        parent: Option<&ChunkRef>,
    ) -> Self {
        Self {
            hash: block.hash(),
            height: block.height().uint(),
            block,
            state,
            logs,
            parent: parent.map(|p| Arc::downgrade(&p.0)).unwrap_or(Weak::new()),
            childs: RwLock::new(Vec::new()),
        }
    }
}

impl ChunkRef {
    pub(super) fn new(
        block: Arc<dyn Block>,
        state: Arc<Box<dyn State>>,
        logs: Arc<dyn Logs>,
        parent: Option<&ChunkRef>,
    ) -> Self {
        Self(Arc::new(Chunk::new(block, state, logs, parent)))
    }

    pub(crate) fn clone_with_replaced_block(&self, block: Arc<dyn Block>) -> Self {
        assert!(
            self.0.childs.read().unwrap().is_empty(),
            "clone_with_replaced_block requires a leaf chunk (no children)"
        );
        let mut obj = Chunk::new(block, self.0.state.clone(), self.0.logs.clone(), None);
        obj.parent = self.0.parent.clone();
        Self(Arc::new(obj))
    }

    pub(crate) fn height(&self) -> u64 {
        self.0.height
    }

    pub(crate) fn hash(&self) -> &Hash {
        &self.0.hash
    }

    pub(crate) fn ptr_eq(&self, other: &ChunkRef) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }

    pub(crate) fn block(&self) -> Arc<dyn Block> {
        self.0.block.clone()
    }

    pub(crate) fn state(&self) -> Arc<Box<dyn State>> {
        self.0.state.clone()
    }

    pub(crate) fn logs(&self) -> Arc<dyn Logs> {
        self.0.logs.clone()
    }

    pub(super) fn parent(&self) -> Option<ChunkRef> {
        self.0.parent.upgrade().map(ChunkRef)
    }

    pub(super) fn children(&self) -> Vec<ChunkRef> {
        self.0
            .childs
            .read()
            .unwrap()
            .iter()
            .cloned()
            .map(ChunkRef)
            .collect()
    }

    pub(super) fn append(&self, child: &ChunkRef, fast_sync: bool) -> Rerr {
        if fast_sync {
            self.0.childs.write().unwrap().push(child.0.clone());
            return Ok(());
        }
        if child.0.parent.as_ptr() != Arc::as_ptr(&self.0) {
            return errf!("child parent mismatch");
        }
        if child.0.height != self.0.height + 1 {
            return errf!(
                "child height need {} but got {}",
                self.0.height + 1,
                child.0.height
            );
        }
        let mut childs = self.0.childs.write().unwrap();
        if childs
            .iter()
            .any(|c| Arc::ptr_eq(c, &child.0) || c.hash == child.0.hash)
        {
            return errf!("repetitive child <{}, {}>", child.0.height, child.0.hash);
        }
        childs.push(child.0.clone());
        Ok(())
    }
}
