

#[derive(Debug, Clone)]
pub struct TxPkg {
    pub data: Arc<Vec<u8>>,
    pub seek: usize,
    pub size: usize,
    pub orgi: TxOrigin,
    pub objc: Box<dyn Transaction>,
    pub hash: Hash,
    pub fpur: u64, // fee purity
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



}
