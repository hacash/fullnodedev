
impl MsgHandler {

    async fn send_hashs(&self, peer: Arc<Peer>, buf: Vec<u8>) {
        crate::core::send_hashs(self, peer, buf).await;
    }

    async fn receive_hashs(&self, peer: Arc<Peer>, buf: Vec<u8>) {
        crate::core::receive_hashs(self, peer, buf).await;
    }

}
