
impl MsgHandler {

    async fn send_blocks(&self, peer: Arc<Peer>, buf: Vec<u8>) {
        crate::core::send_blocks(self, peer, buf).await;
    }

    async fn receive_blocks(&self, peer: Arc<Peer>, buf: Vec<u8>) {
        crate::core::receive_blocks(self, peer, buf).await;
    }

}
