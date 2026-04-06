impl P2PManage {

    pub async fn connect_stable_then_boot(&self) -> Rerr {
        crate::core::connect_stable_then_boot(self).await
    }

    pub async fn connect_stable_nodes(&self) -> Rerr {
        crate::core::connect_stable_nodes(self).await
    }

    pub async fn connect_boot_nodes(&self) -> Rerr {
        crate::core::connect_boot_nodes(self).await
    }

    pub async fn connect_node(&self, addr: SocketAddr) -> Ret<Arc<Peer>> {
        crate::core::connect_node(self, addr).await
    }

    pub async fn handle_conn(&self, conn: TcpStream, report_me: bool) -> Ret<Arc<Peer>> {
        crate::core::handle_conn(self, conn, report_me).await
    }

    pub async fn insert_peer(&self, conn: TcpStream, mynodeinfo: Vec<u8>) -> Ret<Arc<Peer>> {
        crate::core::insert_peer(self, conn, mynodeinfo).await
    }

    pub(crate) fn pick_my_node_info(&self) -> Vec<u8> {
        let mut nodeinfo = vec![0u8; 2 + 2 + PEER_KEY_SIZE * 2];
        nodeinfo.splice(2..4, self.cnf.listen.to_be_bytes());
        nodeinfo.splice(4..20, self.cnf.node_key);
        let mut namebt = self.cnf.node_name.clone();
        namebt += "                ";
        namebt.truncate(PEER_KEY_SIZE);
        nodeinfo.splice(20..20 + PEER_KEY_SIZE, namebt.into_bytes());
        nodeinfo
    }

    async fn delay_close_peer(&self, peer: Option<Arc<Peer>>, delay: u64) {
        if peer.is_none() {
            return
        }
        let peer = peer.unwrap();
        if delay == 0 {
            peer.disconnect();
            return
        }
        tokio::spawn(async move {
            asleep(delay as f32).await;
            peer.disconnect();
        });
    }

    pub(crate) async fn delay_close_peers(&self, peers: Vec<Arc<Peer>>, delay: u64) {
        for peer in peers {
            self.delay_close_peer(Some(peer), delay).await;
        }
    }

    pub(crate) fn load_stable_nodes(&self) -> Vec<SocketAddr> {
        if !self.cnf.use_stable_nodes {
            return Vec::new();
        }
        let max = self.cnf.backbone_peers;
        let mut res = Vec::new();
        if max == 0 {
            return res
        }
        let mut seen = std::collections::HashSet::<SocketAddr>::new();
        for addr in &self.cnf.boot_nodes {
            seen.insert(*addr);
        }
        for p in self.backbones() {
            seen.insert(p.addr);
        }
        for p in self.offshoots() {
            seen.insert(p.addr);
        }
        let path = stable_nodes_path_from_conf(&self.cnf);
        let addrs = read_stable_nodes_file(&path, max);
        for addr in addrs {
            if seen.insert(addr) {
                res.push(addr);
            }
        }
        res
    }
}
