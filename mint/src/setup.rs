use crate::TransactionCoinbase;
use basis::interface::Transaction;
use field::{Field, FromJSON};
use sys::Ret;

fn create_mainnet_prelude_tx(buf: &[u8]) -> Ret<(Box<dyn Transaction>, usize)> {
    let (tx, sk) = TransactionCoinbase::create(buf)?;
    Ok((Box::new(tx), sk))
}

fn decode_mainnet_prelude_tx(json: &str) -> Ret<Box<dyn Transaction>> {
    let mut tx = TransactionCoinbase::default();
    tx.from_json(json)?;
    Ok(Box::new(tx))
}

pub fn register_protocol_extensions(setup: &mut protocol::setup::ProtocolSetup) {
    setup.tx_codec(TransactionCoinbase::TYPE, create_mainnet_prelude_tx, decode_mainnet_prelude_tx);
    crate::action::register(setup)
}

