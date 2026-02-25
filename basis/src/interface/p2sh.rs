
pub trait P2sh : Send + Sync {
    fn code_conf(&self) -> u8 { 0 }
    fn code_stuff(&self) -> &[u8];
    fn witness(&self) -> &[u8];
}
