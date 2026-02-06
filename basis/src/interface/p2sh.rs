
pub trait P2sh : Send + Sync {
    fn code_stuff(&self) -> &[u8];
    fn witness(&self) -> &[u8];
}
