
impl P2PManage {

    pub async fn broadcast_unaware(&self, key: &KnowKey, ty: u16, body: Vec<u8>) {
        let mut resps = vec![];
        let peers = vec![ self.backbones(), self.offshoots() ].concat();
        for peer in peers {
            if !peer.knows.check(key) {
                peer.knows.add(key.clone());
                resps.push(peer);
            }
        }
        let msgbody = vec![ty.to_be_bytes().to_vec(), body].concat();
        let msgbuf = tcp_create_msg(MSG_CUSTOMER, msgbody);
        for peer in resps {
            let _ = peer.send(&msgbuf).await;
        }
    }

}
