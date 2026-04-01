
pub struct P2PManage {
    cnf: NodeConf,
    msghandler: Arc<MsgHandler>,
    backbones: PeerList,
    offshoots: PeerList,
}

impl P2PManage {

    pub fn new(cnf: &NodeConf, msghl: Arc<MsgHandler>) -> P2PManage {
        P2PManage {
            cnf: cnf.clone(),
            msghandler: msghl,
            backbones: StdMutex::new(vec![]).into(),
            offshoots: StdMutex::new(vec![]).into(),
        }
    }

    pub fn all_peer_prints(&self) -> Vec<String> {
        let peers = vec![ self.backbones(), self.offshoots() ].concat();
        let mut prints = Vec::with_capacity(peers.len());
        for p in peers {
            prints.push(p.nick());
        }
        prints
    }

    fn insert(&self, peer: Arc<Peer>) -> Vec<Arc<Peer>> {
        let key = peer.key;
        let mypid = &self.cnf.node_key;
        let mut dropeds = vec![];
        // Remove old peers with same key from both tables before inserting
        take_same_peer_from_dht_vec(&self.backbones, &key, &mut dropeds);
        take_same_peer_from_dht_vec(&self.offshoots, &key, &mut dropeds);
        // Insert new peer
        let mut lmax = self.cnf.offshoot_peers;
        let mut list = self.offshoots.clone();
        if peer.is_public {
            lmax = self.cnf.backbone_peers;
            list = self.backbones.clone();
        }
        let droped = insert_peer_to_dht_list(list, lmax, mypid, peer.clone());
        if droped.is_none() {
            return dropeds
        }
        dropeds.push(droped.unwrap());
        if !peer.is_public {
            return dropeds
        }
        // Try second insert to offshoots for boosted peer
        if dropeds.last().map(|p|p.is_cntome).unwrap_or(false) {
            let last = dropeds.pop().unwrap();
            let dpk = last.key;
            let exist = self.backbones.lock().unwrap().iter().any(|p| p.key == dpk)
                || self.offshoots.lock().unwrap().iter().any(|p| p.key == dpk);
            if !exist {
                if let Some(droped) = insert_peer_to_dht_list(self.offshoots.clone(), self.cnf.offshoot_peers, mypid, last) {
                    dropeds.push(droped);
                }
            } else {
                dropeds.push(last);
            }
        }
        dropeds
    }

    fn publics(&self) -> Vec<Arc<Peer>> {
        let mut resps = vec![];
        let peers = vec![ self.backbones(), self.offshoots() ].concat();
        for p in peers {
            if p.is_public {
                resps.push(p);
            }
        }
        resps
    }

    fn backbones(&self) -> Vec<Arc<Peer>> {
        self.backbones.lock().unwrap().clone()
    }

    fn offshoots(&self) -> Vec<Arc<Peer>> {
        self.offshoots.lock().unwrap().clone()
    }

    async fn disconnect_all_peers(&self) {
        let peers = vec![ self.backbones(), self.offshoots() ].concat();
        for p in peers {
            p.disconnect().await
        }
    }

    fn print_conn_peers(&self) {
        let p1 = self.backbones.lock().unwrap();
        let mut l1names = vec![];
        for li in p1.iter() {
            l1names.push(format!("{}({})", li.nick(), li.key[0..2].to_vec().to_hex()));
        }
        let l1 = p1.len();
        let l2 = self.offshoots.lock().unwrap().len();
        let mykp = self.cnf.node_key[0..2].to_vec().to_hex();
        flush!("[P2P] {} public {} subnet nodes connected, key({}) => {}.\n",
            l1, l2, mykp, l1names.join(", "));
    }

    pub fn exit(&self) {
    }

}
