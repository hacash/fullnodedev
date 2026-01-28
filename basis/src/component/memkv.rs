

pub type MemMap = HashMap<Vec<u8>, Option<Vec<u8>>>;



#[derive(Default, Clone)]
pub struct MemKV {
    pub memry: MemMap
}

impl MemDB for MemKV {

    fn new() -> MemKV {
        Self {
            memry: HashMap::default()
        }
    }

    fn len(&self) -> usize {
        self.memry.len()
    }

    fn del(&mut self, k: Vec<u8>) {
        self.memry.insert(k, None);
    }
    
    fn put(&mut self, k: Vec<u8>, v: Vec<u8>) {
        self.memry.insert(k, Some(v));
    }
    
    fn get(&self, k: &Vec<u8>) -> Option<&Option<Vec<u8>>> {
        self.memry.get(k)
    }

    fn for_each(&self, each: &mut dyn FnMut(&Vec<u8>, &Option<Vec<u8>>)) {
        for (k, v) in self.memry.iter() {
            each(k, v);
        }
    }

}
