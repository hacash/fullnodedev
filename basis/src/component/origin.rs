


#[derive(Debug, Default, PartialEq, Copy, Clone)]
pub enum BlkOrigin {
    #[default] Unknown, 
    Rebuild,
    Sync,
    Discover, // other find
    Mint,     // mine miner find
}


#[derive(Debug, Default, PartialEq, Copy, Clone)]
pub enum TxOrigin {
    #[default] Unknown,
    Sync,
    Broadcast, // other find
    Submit,    // mine miner find
}

