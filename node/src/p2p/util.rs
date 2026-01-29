
/**
 * ipport(6bytes) + key(16byte)
 */
fn serialize_public_nodes(peerlist: &Vec<Arc<Peer>>, _max: usize) -> (usize, Vec<u8>) {
    let mut listbts = vec![];
    let mut count = 0usize;
    for p in peerlist {
        if !p.is_public || !p.addr.is_ipv4() {
            continue
        }
        let ipbts = match p.addr.ip() {
            IpAddr::V4(ip) => ip.octets(),
            _ => continue,
        };
        listbts.push(vec![
            ipbts.to_vec(),
            p.addr.port().to_be_bytes().to_vec(),
            p.key.to_vec(),
        ].concat());
        count+=1;
        if count >= 200 {
            break // end max
        }
    }
    (count, listbts.concat())
}


fn parse_public_nodes(bts: &[u8]) -> Vec<(PeerKey, SocketAddr)> {
    let sn = 4 + 2 + 16; // ip port key
    let num = bts.len() / sn;
    let mut addr = Vec::with_capacity(num);
    for i in 0..num {
        let one = &bts[i*sn .. i*sn+sn];
        let ip: [u8;4] = one[0..4].try_into().unwrap();
        let port: [u8;2] = one[4..6].try_into().unwrap() ;
        let key: [u8;16] = one[6..22].try_into().unwrap() ;
        addr.push((key, SocketAddr::new(
            IpAddr::from(ip), 
            u16::from_be_bytes(port)
        )));
    }
    addr
}


fn stable_nodes_path_from_conf(cnf: &NodeConf) -> PathBuf {
    join_path(&cnf.data_dir, "stable.nodes")
}


fn read_stable_nodes_file(path: &PathBuf, max: usize) -> Vec<SocketAddr> {
    if max == 0 {
        return vec![];
    }
    let Ok(content) = std::fs::read_to_string(path) else {
        return vec![];
    };
    let mut res: Vec<SocketAddr> = Vec::new();
    let mut seen = std::collections::HashSet::<SocketAddr>::new();
    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let Ok(addr) = line.parse::<SocketAddr>() else {
            continue;
        };
        if seen.insert(addr) {
            res.push(addr);
            if res.len() >= max {
                break;
            }
        }
    }
    res
}


fn persist_stable_nodes_file(path: &PathBuf, peers: &PeerList, max: usize) {
    let mut out = String::new();
    if max > 0 {
        let list = peers.lock().unwrap();
        let mut count = 0usize;
        for p in list.iter() {
            if count >= max {
                break;
            }
            if !p.is_public {
                continue;
            }
            out.push_str(&format!("{}\n", p.addr));
            count += 1;
        }
    }
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let mut tmp_path = path.clone();
    tmp_path.set_extension("nodes.tmp");
    if std::fs::write(&tmp_path, out).is_ok() {
        let _ = std::fs::rename(&tmp_path, path);
    } else {
        let _ = std::fs::remove_file(&tmp_path);
    }
}


fn persist_stable_nodes_from_conf(cnf: &NodeConf, peers: &PeerList) {
    if !cnf.use_stable_nodes {
        return;
    }
    let path = stable_nodes_path_from_conf(cnf);
    persist_stable_nodes_file(&path, peers, cnf.backbone_peers);
}
