
pub struct EmptyLogs {}
impl Logs for EmptyLogs {}


pub struct BlockLogs {
    bhei: u64,
    logs: Vec<Vec<u8>>,
    disk: Arc<dyn DiskDB>
}


impl Logs for BlockLogs {
    fn push(&mut self, stuff: &dyn Serialize) {
        if self.bhei == 0 {
            return // not save
        }
        self.logs.push(stuff.serialize())
    }

    fn load(&self, hei: u64, idx: usize) -> Option<Vec<u8>> {
        let key = Self::wnk(hei, idx);
        self.disk.read(&key)
    }

    fn remove(&self, height: u64) {
        let hei = Uint8::from(height);
        let lnk = [hei.serialize(), b"n".to_vec()].concat();
        let num = self.read_len(&lnk);
        for i in 0 .. num {
            let k = Self::wnk(height, i);
            self.disk.remove(&k);
        }
        self.disk.remove(&lnk);
    }

    fn height(&self) -> u64 {
        self.bhei
    }

    fn write_to_disk(&self) {
        let m = self.logs.len();
        for i in 0 .. m {
            self.disk.save(&self.nk(i), &self.logs[i]);
        }
        if m > 0 {
            self.update_len(self.logs.len());
        }
    }

    fn snapshot_len(&self) -> usize {
        self.logs.len()
    }

    fn truncate(&mut self, len: usize) {
        self.logs.truncate(len);
    }

}


impl BlockLogs {

    fn lnk(&self) -> Vec<u8> {
        let hei = Uint8::from(self.bhei);
        [hei.serialize(), b"n".to_vec()].concat()
    }

    fn wnk(hei: u64, idx: usize) -> Vec<u8> {
        let hei = Uint8::from(hei);
        let num = Uint8::from(idx as u64);
        [hei.serialize(), num.serialize()].concat()
    }

    fn nk(&self, n: usize) -> Vec<u8> {
        Self::wnk(self.bhei, n)
    }

    pub fn wrap(disk: Arc<dyn DiskDB>) -> Self {
        Self { disk, bhei: 0, logs: Vec::new() }
    }

    pub fn next(&self, hei: u64) -> Self {
        Self { disk: self.disk.clone(), bhei: hei, logs: Vec::new() }
    }

}

impl BlockLogs {

    fn read_len(&self, lnk: &Vec<u8>) -> usize {
        let mut num = Uint8::from(0);
        match self.disk.read(lnk) {
            None => num,
            Some(v) => {
                num.parse(&v).unwrap(); // must
                num
            }
        }.uint() as usize
    }

    pub fn len(&self) -> usize {
        let l = self.logs.len();
        if l > 0 {
            return l
        }
        self.read_len(&self.lnk())
    }

    // load
    pub fn read(&self, idx: usize) -> Option<Vec<u8>> {
        if idx < self.logs.len() {
            return Some(self.logs[idx].clone())
        }
        let i = &self.nk(idx);
        self.disk.read(i)
    }


    // write

    fn update_len(&self, n: usize) {
        let num = Uint8::from(n as u64);
        self.disk.save(&self.lnk(), &num.serialize());
    }


}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[derive(Default)]
    struct MemDisk {
        kv: Mutex<HashMap<Vec<u8>, Vec<u8>>>,
    }

    impl DiskDB for MemDisk {
        fn read(&self, key: &[u8]) -> Option<Vec<u8>> {
            self.kv.lock().unwrap().get(key).cloned()
        }

        fn save(&self, key: &[u8], val: &[u8]) {
            self.kv.lock().unwrap().insert(key.to_vec(), val.to_vec());
        }

        fn remove(&self, key: &[u8]) {
            self.kv.lock().unwrap().remove(key);
        }

        fn for_each(&self, each: &mut dyn FnMut(&[u8], &[u8])->bool) -> Result<(), String> {
            let rows: Vec<(Vec<u8>, Vec<u8>)> = self
                .kv
                .lock()
                .unwrap()
                .iter()
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect();
            for (k, v) in rows {
                if !each(&k, &v) {
                    break
                }
            }
            Ok(())
        }
    }

    #[test]
    fn remove_clears_items_and_length_key() {
        let disk: Arc<dyn DiskDB> = Arc::new(MemDisk::default());
        let height = 88u64;

        let mut wr = BlockLogs::wrap(disk.clone()).next(height);
        wr.push(&Uint1::from(7));
        wr.push(&Uint1::from(8));
        wr.write_to_disk();

        let rd = BlockLogs::wrap(disk.clone()).next(height);
        assert_eq!(rd.len(), 2);
        assert!(rd.load(height, 0).is_some());
        rd.remove(height);

        let after = BlockLogs::wrap(disk.clone()).next(height);
        assert_eq!(after.len(), 0);
        assert!(after.load(height, 0).is_none());

        after.remove(height);
        let final_view = BlockLogs::wrap(disk).next(height);
        assert_eq!(final_view.len(), 0);
    }
}
