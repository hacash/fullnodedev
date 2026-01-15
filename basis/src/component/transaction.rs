

pub struct TxPkg {
    data: Arc<Vec<u8>>,
    seek: usize,
    size: usize,
    pub orgi: TxOrigin,
    pub objc: Box<dyn Transaction>,
    pub hash: Hash,
    pub fpur: u64, // fee purity
}
    
impl_pkg_common!{ TxPkg, Transaction, TxOrigin }


