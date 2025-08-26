

pub struct NilKV {
}


impl NilKV {

    pub fn new() -> Self {
        NilKV {}
    }

}


impl DiskDB for NilKV {}