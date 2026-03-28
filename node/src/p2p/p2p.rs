type PeerSnap = Arc<PeerTableSnap>;
type PeerSnapTx = watch::Sender<PeerSnap>;
type PeerSnapRx = watch::Receiver<PeerSnap>;
type PeerTableCmdTx = mpsc::UnboundedSender<PeerTableCmd>;
type PeerTableCmdRx = mpsc::UnboundedReceiver<PeerTableCmd>;

struct PeerTableSnap {
    backbones: Vec<Arc<Peer>>,
    offshoots: Vec<Arc<Peer>>,
}

impl PeerTableSnap {

    fn new(backbones: Vec<Arc<Peer>>, offshoots: Vec<Arc<Peer>>) -> PeerTableSnap {
        PeerTableSnap {
            backbones,
            offshoots,
        }
    }

}

enum PeerTableCmd {
    Insert(Arc<Peer>, oneshot::Sender<Vec<Arc<Peer>>>),
    Remove(Arc<Peer>),
    BoostPublic(oneshot::Sender<Vec<Arc<Peer>>>),
}


// #[derive(Clone)]
pub struct P2PManage {
    cnf: NodeConf,
    msghandler: Arc<MsgHandler>,
    // 
    peertabletx: PeerTableCmdTx,
    peertablerx: StdMutex<Option<PeerTableCmdRx>>,
    peersnaptx: StdMutex<Option<PeerSnapTx>>,
    peersnaprx: PeerSnapRx,
    // close mark
    // exiter: Exiter,
}

impl P2PManage {

    pub fn new(cnf: &NodeConf, msghl: Arc<MsgHandler>) -> P2PManage {
        let (peertabletx, peertablerx) = mpsc::unbounded_channel();
        let peersnap = Arc::new(PeerTableSnap::new(vec![], vec![]));
        let (peersnaptx, peersnaprx) = watch::channel(peersnap);
        P2PManage {
            cnf: cnf.clone(),
            msghandler: msghl,
            peertabletx,
            peertablerx: Some(peertablerx).into(),
            peersnaptx: Some(peersnaptx).into(),
            peersnaprx,
            // closech: StdMutex::new(Some(closerx)),
            // exiter: Exiter::new(),
        }
    }

    fn start_peer_table_loop(&self) {
        let mayrx = self.peertablerx.lock().unwrap().take();
        let maytx = self.peersnaptx.lock().unwrap().take();
        if mayrx.is_none() || maytx.is_none() {
            return
        }
        let mut peertablerx = mayrx.unwrap();
        let peersnaptx = maytx.unwrap();
        let cnf = self.cnf.clone();
        tokio::spawn(async move {
            let mut backbones = vec![];
            let mut offshoots = vec![];
            while let Some(cmd) = peertablerx.recv().await {
                let (changed, persist) = match cmd {
                    PeerTableCmd::Insert(peer, backtx) => {
                        let (dropeds, bchanged) = insert_peer_to_tables(&cnf, &mut backbones, &mut offshoots, peer);
                        let _ = backtx.send(dropeds);
                        (true, bchanged)
                    },
                    PeerTableCmd::Remove(peer) => {
                        let r1 = remove_peer_from_dht_vec(&mut offshoots, &peer);
                        let r2 = remove_peer_from_dht_vec(&mut backbones, &peer);
                        (r1 || r2, r2)
                    },
                    PeerTableCmd::BoostPublic(backtx) => {
                        let (dropeds, changed2) = boost_public_to_tables(&cnf, &mut backbones, &mut offshoots);
                        let _ = backtx.send(dropeds);
                        (changed2, changed2)
                    },
                };
                if changed {
                    let peersnap = Arc::new(PeerTableSnap::new(backbones.clone(), offshoots.clone()));
                    let _ = peersnaptx.send(peersnap);
                }
                if persist {
                    persist_stable_nodes_async(cnf.clone(), backbones.clone());
                }
            }
        });
    }

    fn peer_snapshot(&self) -> PeerSnap {
        self.peersnaprx.borrow().clone()
    }

    pub fn all_peer_prints(&self) -> Vec<String> { 
        let peersnap = self.peer_snapshot();
        let mut prints = Vec::with_capacity(peersnap.backbones.len() + peersnap.offshoots.len());
        for p in peersnap.backbones.iter().chain(peersnap.offshoots.iter()) {
            prints.push(p.nick());
        }
        prints
    }

    async fn insert(&self, peer: Arc<Peer>) -> Ret<Vec<Arc<Peer>>> {
        let (backtx, backrx) = oneshot::channel();
        if self.peertabletx.send(PeerTableCmd::Insert(peer, backtx)).is_err() {
            return errf!("peer table loop closed")
        }
        match backrx.await {
            Ok(dropeds) => Ok(dropeds),
            Err(_) => errf!("peer table loop closed"),
        }
    }

    async fn boost_public_table(&self) -> Vec<Arc<Peer>> {
        let (backtx, backrx) = oneshot::channel();
        if self.peertabletx.send(PeerTableCmd::BoostPublic(backtx)).is_err() {
            return vec![]
        }
        match backrx.await {
            Ok(dropeds) => dropeds,
            Err(_) => vec![],
        }
    }

    fn publics(&self) -> Vec<Arc<Peer>> {
        let mut resps = vec![];
        let peersnap = self.peer_snapshot();
        for p in peersnap.backbones.iter().chain(peersnap.offshoots.iter()) {
            if p.is_public {
                resps.push(p.clone());
            }
        }
        resps
    }

    fn backbones(&self) -> Vec<Arc<Peer>> {
        self.peer_snapshot().backbones.clone()
    }

    fn offshoots(&self) -> Vec<Arc<Peer>> {
        self.peer_snapshot().offshoots.clone()
    }

    async fn disconnect_all_peers(&self) {
        let peersnap = self.peer_snapshot();
        for p in peersnap.backbones.iter().chain(peersnap.offshoots.iter()) {
            p.disconnect().await
        }
    }

    fn print_conn_peers(&self) {
        let peersnap = self.peer_snapshot();
        let mut l1names = vec![];
        for li in peersnap.backbones.iter() {
            l1names.push(format!("{}({})", li.nick(), li.key[0..2].to_vec().to_hex()));
        }
        let l1 = peersnap.backbones.len();
        let l2 = peersnap.offshoots.len();
        let mykp = self.cnf.node_key[0..2].to_vec().to_hex();
        flush!("[P2P] {} public {} subnet nodes connected, key({}) => {}.\n", 
            l1, l2, mykp, l1names.join(", "));
    }

    pub fn exit(&self) {
        // self.exiter.exit();
    }

}

fn take_same_peer_from_dht_vec(list: &mut Vec<Arc<Peer>>, key: &PeerKey, dropeds: &mut Vec<Arc<Peer>>) {
    list.retain(|p| {
        if p.key == *key {
            dropeds.push(p.clone());
            return false
        }
        true
    });
}

fn insert_peer_to_tables(cnf: &NodeConf, backbones: &mut Vec<Arc<Peer>>, offshoots: &mut Vec<Arc<Peer>>, peer: Arc<Peer>) -> (Vec<Arc<Peer>>, bool) {
    let mypid = &cnf.node_key;
    let key = peer.key;
    let mut dropeds = vec![];
    let mut bchanged = false;
    let backlen = backbones.len();
    take_same_peer_from_dht_vec(backbones, &key, &mut dropeds);
    if backbones.len() != backlen {
        bchanged = true;
    }
    take_same_peer_from_dht_vec(offshoots, &key, &mut dropeds);
    if peer.is_public {
        bchanged = true;
        let droped = insert_peer_to_dht_vec(backbones, cnf.backbone_peers, mypid, peer.clone());
        if droped.is_none() {
            return (dropeds, bchanged)
        }
        let droped = droped.unwrap();
        if !droped.is_cntome {
            dropeds.push(droped);
            return (dropeds, bchanged)
        }
        let dpk = droped.key;
        let exist = backbones.iter().any(|p| p.key == dpk)
            || offshoots.iter().any(|p| p.key == dpk);
        if exist {
            dropeds.push(droped);
            return (dropeds, bchanged)
        }
        if let Some(droped) = insert_peer_to_dht_vec(offshoots, cnf.offshoot_peers, mypid, droped) {
            dropeds.push(droped);
        }
        return (dropeds, bchanged)
    }
    if let Some(droped) = insert_peer_to_dht_vec(offshoots, cnf.offshoot_peers, mypid, peer) {
        dropeds.push(droped);
    }
    (dropeds, bchanged)
}

fn boost_public_to_tables(cnf: &NodeConf, backbones: &mut Vec<Arc<Peer>>, offshoots: &mut Vec<Arc<Peer>>) -> (Vec<Arc<Peer>>, bool) {
    if backbones.len() >= cnf.backbone_peers {
        return (vec![], false)
    }
    let peer = match checkout_one_from_dht_vec(offshoots, |p|p.is_public) {
        Some(peer) => peer,
        None => return (vec![], false),
    };
    let (dropeds, _) = insert_peer_to_tables(cnf, backbones, offshoots, peer);
    (dropeds, true)
}
