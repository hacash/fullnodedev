


pub type MemMap = HashMap<Vec<u8>, Option<Vec<u8>>>;



pub type FnBuildDB = fn(_: &PathBuf)->Box<dyn DiskDB>;


