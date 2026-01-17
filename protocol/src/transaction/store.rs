


pub fn build_tx_package(data: Vec<u8>) -> Ret<TxPkg> {
    let (objc, sk) = transaction::create(&data)?;
    let pkg = TxPkg {
        orgi: TxOrigin::Unknown,
        hash: objc.hash(),
        fpur: objc.fee_purity(),
        data: data.into(),
        seek: 0,
        size: sk,
        objc,
    };
    Ok(pkg)
}


