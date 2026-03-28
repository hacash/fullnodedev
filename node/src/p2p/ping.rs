

/**
* Find and Connect new public node
*/
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
            return // unnecessary
        }
        let dropeds = self.boost_public_table().await;
        self.delay_close_peers(dropeds, 15).await;
    }

    
}

/**
* check no active
*/
async fn do_check_active(peers: Vec<Arc<Peer>>) {
    let now = SystemTime::now();
    // println!("do_check_active num = {}", peers.len());
    for peer in peers {
        let active = { peer.active.lock().unwrap().clone() };
        if now - secs(60*20) > active { // 20min
            // disconnect unactive peers
            // println!("disconnect unactive peers {}", peer.nick());
            peer.disconnect().await;
        } 
    }
}


/**
* ping
*/
async fn do_ping_nodes(peers: Vec<Arc<Peer>>) {
    // just ping public nodes
    let now = SystemTime::now();
    for peer in peers {
        let active = { peer.active.lock().unwrap().clone() };
        if now - secs(60*5) > active { // 5min
            // send ping msg
            let _ = peer.send_p2p_msg(MSG_PING, vec![]).await;
        } 
    }
}
