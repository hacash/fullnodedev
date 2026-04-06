
pub type FnPreludeTxCreateFunc = fn(&[u8]) -> Ret<(Box<dyn Transaction>, usize)>;
pub type FnPreludeTxJsonDecodeFunc = fn(&str) -> Ret<Box<dyn Transaction>>;

pub struct PreludeTxCodec {
    pub create: FnPreludeTxCreateFunc,
    pub json_decode: FnPreludeTxJsonDecodeFunc,
}
