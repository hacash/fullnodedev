

struct RawDB {
    ptr: *mut leveldb_t,
}

impl Drop for RawDB {
    fn drop(&mut self) {
        unsafe {
            leveldb_close(self.ptr);
        }
    }
}

unsafe impl Send for RawDB {}
unsafe impl Sync for RawDB {}

struct RawReadOptions {
    ptr: *mut leveldb_readoptions_t,
}

impl Drop for RawReadOptions {
    fn drop(&mut self) {
        unsafe {
            leveldb_readoptions_destroy(self.ptr);
        }
    }
}

unsafe impl Send for RawReadOptions {}
unsafe impl Sync for RawReadOptions {}

struct RawWriteOptions {
    ptr: *mut leveldb_writeoptions_t,
}

impl Drop for RawWriteOptions {
    fn drop(&mut self) {
        unsafe {
            leveldb_writeoptions_destroy(self.ptr);
        }
    }
}

unsafe impl Send for RawWriteOptions {}
unsafe impl Sync for RawWriteOptions {}

struct RawIter {
    ptr: *mut leveldb_iterator_t,
}

impl Drop for RawIter {
    fn drop(&mut self) {
        unsafe {
            leveldb_iter_destroy(self.ptr);
        }
    }
}


pub struct LevelDB {
    database: RawDB,
    read_options: RawReadOptions,
    write_options: RawWriteOptions,
    // ldb: LevelDatabase<LDBKey>,
}


impl LevelDB {

    pub fn open(dir: &Path) -> LevelDB {
        // let mut opts = Options::new();
        // opts.create_if_missing = true;
        // let ldb = LevelDatabase::open(dir, opts).unwrap();
        let mut error = ptr::null_mut();
        let database = unsafe {
            let c_options = leveldb_options_create();
            leveldb_options_set_create_if_missing(c_options, 1u8);
            let c_dbpath = CString::new(dir.to_str().unwrap()).unwrap();
            let db = leveldb_open(c_options as *const leveldb_options_t,
                c_dbpath.as_bytes_with_nul().as_ptr() as *const c_char,
                                  &mut error);
            leveldb_options_destroy(c_options);
            db
        };
        if error != ptr::null_mut() {
            let err = new_string_from_char_ptr(error);
            panic!("{}", err)
        }
        let read_options = unsafe {
            RawReadOptions { ptr: leveldb_readoptions_create() }
        };
        let write_options = unsafe {
            let ptr = leveldb_writeoptions_create();
            leveldb_writeoptions_set_sync(ptr, if db_sync_enabled() { 1 } else { 0 });
            RawWriteOptions { ptr }
        };
        // create
        LevelDB{
            database: RawDB { ptr: database },
            read_options,
            write_options,
        }
    }

    // get if find, bool is not check base
    
    pub fn get_at(&self, k: &[u8]) -> Option<RawBytes> {
        let mut error = ptr::null_mut();
        let mut length: size_t = 0;
        let result = unsafe {
            let res = leveldb_get(self.database.ptr,
                self.read_options.ptr,
                k.as_ptr() as *mut c_char,
                k.len() as size_t,
                &mut length,
                &mut error);
            res
        };
        if error != ptr::null_mut() {
            let err = new_string_from_char_ptr(error);
            panic!("{}", err)
        }
        if result.is_null() {
            return None // not find
        }
        Some(unsafe {
            RawBytes::from_raw_unchecked(result as *mut u8, length)
        })
    }
    
    
    pub fn get(&self, k: &[u8]) -> Option<Vec<u8>> {
        if let Some(v) = self.get_at(k) {
            return Some(v.into())
        }
        None
    }

    // set
    
    pub fn put(&self, k: &[u8], value: &[u8]) {
        let mut error = ptr::null_mut();
        unsafe {
            leveldb_put(self.database.ptr,
                self.write_options.ptr,
                k.as_ptr() as *mut c_char,
                k.len() as size_t,
                value.as_ptr() as *mut c_char,
                value.len() as size_t,
                &mut error);
        }
        if error != ptr::null_mut() {
            let err = new_string_from_char_ptr(error);
            panic!("{}", err)
        }
    }

    // del
    
    pub fn rm(&self, k: &[u8]) {
        let mut error = ptr::null_mut();
        unsafe {
            leveldb_delete(self.database.ptr,
                self.write_options.ptr,
                k.as_ptr() as *mut c_char,
                k.len() as size_t,
                &mut error);
        }
        if error != ptr::null_mut() {
            let err = new_string_from_char_ptr(error);
            panic!("{}", err)
        }
    }
    
    // write batch
    
    pub fn write(&self, batch: &Writebatch) {
        let mut error = ptr::null_mut();
        unsafe {
            leveldb_write(self.database.ptr,
                          self.write_options.ptr,
                          batch.ptr,
                          &mut error);
        }
        if error != ptr::null_mut() {
            let err = new_string_from_char_ptr(error);
            panic!("{}", err)
        }
    }

    pub fn for_each(&self, each: &mut dyn FnMut(&[u8], &[u8])->bool) -> Rerr{
        let iter = unsafe {
            let ptr = leveldb_create_iterator(self.database.ptr, self.read_options.ptr);
            leveldb_iter_seek_to_first(ptr);
            RawIter { ptr }
        };
        loop {
            if unsafe { leveldb_iter_valid(iter.ptr) } == 0 {
                break
            }
            let mut klen: size_t = 0;
            let mut vlen: size_t = 0;
            let (kptr, vptr) = unsafe {
                (
                    leveldb_iter_key(iter.ptr, &mut klen),
                    leveldb_iter_value(iter.ptr, &mut vlen),
                )
            };
            let (k, v) = unsafe {
                (
                    ::std::slice::from_raw_parts(kptr as *const u8, klen as usize),
                    ::std::slice::from_raw_parts(vptr as *const u8, vlen as usize),
                )
            };
            if !each(k, v) {
                break
            }
            unsafe {
                leveldb_iter_next(iter.ptr);
            }
        }
        let mut error: *const c_char = ptr::null();
        unsafe {
            leveldb_iter_get_error(iter.ptr, &mut error as *mut *const c_char as *const *const c_char);
        }
        if !error.is_null() {
            return Err(new_string_from_char_ptr(error))
        }
        Ok(())
    }


}
