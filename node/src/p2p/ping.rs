
impl P2PManage {

    async fn ping_nodes(&self) {
        do_ping_nodes(self.backbones()).await;
    }

    async fn check_active_nodes(&self) {
        do_check_active(self.backbones()).await;
        do_check_active(self.offshoots()).await;
    }

    async fn boost_public(&self) {
        let chn = self.backbones().len();
        if chn >= self.cnf.backbone_peers {
            return
        }
        let remv = checkout_one_from_dht_list(self.offshoots.clone(), |p|p.is_public);
        if remv.is_none() {
            return
        }
        let peer = remv.unwrap();
        let dropeds = self.insert(peer);
        self.delay_close_peers(dropeds, 15).await;
    }

}

async fn do_check_active(peers: Vec<Arc<Peer>>) {
    let now = SystemTime::now();
    for peer in peers {
        let active = { peer.active.lock().unwrap().clone() };
        if now - secs(60*20) > active {
            peer.disconnect().await;
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
