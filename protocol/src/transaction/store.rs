


pub fn build_tx_package(data: Vec<u8>) -> Ret<TxPkg> {
    let (objc, sk) = transaction::transaction_create(&data)?;
    let data = maybe!(sk == data.len(), data, data[..sk].to_vec());
    Ok(TxPkg::new(objc, data))
}
