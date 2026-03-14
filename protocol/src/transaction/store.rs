


pub fn build_tx_package(data: Vec<u8>) -> Ret<TxPkg> {
    let (objc, sk) = transaction::transaction_create(&data)?;
    if sk != data.len() {
        return errf!(
            "transaction parse length mismatch: consumed {} but input length is {}",
            sk,
            data.len()
        )
    }
    Ok(TxPkg::new(objc, data))
}
