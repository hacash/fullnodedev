pub type FnTxCreateFunc = fn(&[u8]) -> Ret<(Box<dyn Transaction>, usize)>;
pub type FnTxJsonDecodeFunc = fn(&str) -> Ret<Box<dyn Transaction>>;

#[derive(Clone, Copy)]
pub struct TxCodec {
    pub create: FnTxCreateFunc,
    pub json_decode: FnTxJsonDecodeFunc,
}
