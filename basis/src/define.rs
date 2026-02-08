



/// Deprecated: was used as tx-size quantization granule (32 bytes per "gas standard compute unit").
/// fee_purity is now per-byte (fee238 / txSize). Kept only for reference.
#[deprecated(note = "fee_purity is now per-byte; GSCU division removed")]
pub const GSCU: u64 = 32;

pub const P2P_MSG_TX_SUBMIT:           u16 = 7; // new tx    arrived
