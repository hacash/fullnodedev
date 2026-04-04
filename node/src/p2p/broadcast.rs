
impl P2PManage {

    pub async fn broadcast_unaware(&self, key: &KnowKey, ty: u16, body: Vec<u8>) {
        crate::core::broadcast_unaware(self, key, ty, body).await;
    }

}
