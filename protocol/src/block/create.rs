
pub fn create(buf: &[u8]) -> Ret<(Box<dyn Block>, usize)> {
    // println!("block::create {}", hex::encode(buf));
    let version = bufeatone(buf)?;
    match version {
        BlockV1::VERSION => {
            let (blk, mvsk) = BlockV1::create(buf)?;
            Ok((Box::new(blk), mvsk))
        }
        _ => errf!("block version '{}' not find", version)
    }
}


pub fn build_block_package(data: Vec<u8>) -> Ret<BlkPkg> {
    let (objc, _) = create(&data)?;
    Ok(BlkPkg::new(objc, data))
}

