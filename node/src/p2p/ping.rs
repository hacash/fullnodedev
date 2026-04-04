
impl P2PManage {

    pub(crate) async fn ping_nodes(&self) {
        do_ping_nodes(self.backbones()).await;
    }

    pub(crate) async fn check_active_nodes(&self) {
        do_check_active(self.backbones()).await;
        do_check_active(self.offshoots()).await;
    }

    pub(crate) async fn boost_public(&self) {
        let chn = self.backbones().len();
        if chn >= self.cnf.backbone_peers {
            return
        }
        let dropeds = self.boost_public_table().await;
        self.delay_close_peers(dropeds, 15).await;
    }

}

async fn do_check_active(peers: Vec<Arc<Peer>>) {
    let now = SystemTime::now();
    for peer in peers {
        let active = { peer.active.lock().unwrap().clone() };
        if now - secs(60*20) > active {
            peer.disconnect();
        }
    }
}

async fn do_ping_nodes(peers: Vec<Arc<Peer>>) {
    let now = SystemTime::now();
    for peer in peers {
        let active = { peer.active.lock().unwrap().clone() };
        if now - secs(60*5) > active {
            let _ = peer.send_p2p_msg(MSG_PING, vec![]).await;
        }
    }
}
