use std::collections::*;

pub type MemMap = HashMap<Vec<u8>, Option<Vec<u8>>>;



#[derive(Default)]
pub struct MemKV {
    pub memry: MemMap
}

impl MemDB for MemKV {

    fn new() -> MemKV {
        Self {
            memry: HashMap::default()
        }
    }

    fn del(&mut self, k: Vec<u8>) {
        // self.batch.delete(&k);
        self.memry.insert(k, None);
    }
    
    fn put(&mut self, k: Vec<u8>, v: Vec<u8>) {
        // self.batch.put(&k, &v);
        self.memry.insert(k, Some(v));
    }
    
    fn get(&self, k: &Vec<u8>) -> Option<Option<Vec<u8>>> {
        match self.memry.get(k) {
            None => None,
            Some(item) => Some(item.clone()),
        }
    }

    fn to_batch(&self) -> Box<dyn Any> {
        let mut batch = Writebatch::new();
        for (k, v) in self.memry.iter() {
            match v {
                None => batch.delete(k),
                Some(v) => batch.put(k, &v),
            };
        }
        Box::new(batch)
    }

}


/**************************************************** */


pub struct Membatch {
    batch: Writebatch
}

impl MemBatch for Membatch {

    fn new() -> Membatch {
        Self {
            batch: Writebatch::new()
        }
    }

    fn del(&mut self, k: &[u8]) {
        // self.batch.delete(&k);
        self.batch.delete(k);
    }
    
    fn put(&mut self, k: &[u8], v: &[u8]) {
        // self.batch.put(&k, &v);
        self.batch.put(k, v);
    }
}

impl Membatch {

    fn into_batch(self) -> Writebatch {
        self.batch
    }

}