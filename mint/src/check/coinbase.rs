
fn verify_coinbase(height: u64, cbtx: &dyn TransactionRead) -> Rerr {
    let got = cbtx.reward();
    let need = genesis::block_reward(height);
    if need != *got {
        return errf!("block coinbase reward expected {} but got {}", need, got)
    }
    // ok    
    Ok(())
}
