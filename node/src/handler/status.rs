
impl MsgHandler {

    async fn send_status(&self, peer: Arc<Peer>) {
        crate::core::send_status(self, peer).await;
    }

    async fn receive_status(&self, peer: Arc<Peer>, buf: Vec<u8>) {
        crate::core::receive_status(self, peer, buf).await;
    }

}
