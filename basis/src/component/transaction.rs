

#[derive(Debug, Clone)]
pub struct TxPkg {
    data: Arc<Vec<u8>>,
    seek: usize,
    size: usize,
    orgi: TxOrigin,
    objc: Box<dyn Transaction>,
    hash: Hash,
    fpur: u64, // fee purity
}
    
impl_pkg_common!{ TxPkg, Transaction, TxOrigin }


impl TxPkg {

    pub fn new(objc: Box<dyn Transaction>, data: Vec<u8>) -> Self {
        let size = data.len();
        Self {
            orgi: TxOrigin::Unknown,
            hash: objc.hash(),
            fpur: objc.fee_purity(),
            data: data.into(),
            seek: 0,
            size,
            objc,
        }
    }

    pub fn objc(&self) -> &dyn Transaction {
        self.objc.as_ref()
    }

    pub fn objc_box(&self) -> &Box<dyn Transaction> {
        &self.objc
    }

    pub fn hash(&self) -> Hash {
        self.hash
    }

    pub fn fpur(&self) -> u64 {
        self.fpur
    }

}
