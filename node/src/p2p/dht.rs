fn insert_nearest_to_dht_list(
    list: &mut Vec<PeerKey>,
    compare: &PeerKey,
    least: &PeerKey,
    insert: &PeerKey,
) -> bool {
    if 1 != compare_peer_id_topology_distance(compare, insert, least) {
        return false;
    }
    let lenght = list.len();
    if 0 == lenght {
        list.push(*insert);
        return true;
    }
    let mut istidx = lenght;
    for i in 0..lenght {
        let disnum = compare_peer_id_topology_distance(compare, insert, &list[i]);
        if disnum == 1 {
            istidx = i;
            break;
        }
    }
    list.insert(istidx, *insert);
    true
}

fn remove_peer_from_dht_vec(list: &mut Vec<Arc<Peer>>, peer: &Arc<Peer>) -> bool {
    let peer_id = peer.id;
    let old_len = list.len();
    list.retain(|p| p.id != peer_id);
    list.len() != old_len
}

fn find_peer_from_dht_vec(list: &Vec<Arc<Peer>>, pk: &PeerKey) -> Option<Arc<Peer>> {
    list.iter().find(|a| *pk == a.key).map(|a| a.clone())
}

fn insert_peer_to_dht_vec(
    list: &mut Vec<Arc<Peer>>,
    max: usize,
    compare: &PeerKey,
    peer: Arc<Peer>,
) -> Option<Arc<Peer>> {
    let length = list.len();
    let mut insert_idx = length;
    for i in 0..length {
        let disnum = compare_peer_id_topology_distance(compare, &peer.key, &list[i].key);
        if disnum == 1 {
            insert_idx = i;
            break;
        }
    }
    list.insert(insert_idx, peer);
    if list.len() > max {
        return list.pop();
    }
    None
}

fn checkout_one_from_dht_vec<F>(list: &mut Vec<Arc<Peer>>, choose: F) -> Option<Arc<Peer>>
where
    F: Fn(&Peer) -> bool,
{
    let mut rmid = None;
    for i in 0..list.len() {
        if choose(&list[i]) {
            rmid = Some(i);
            break;
        }
    }
    rmid.map(|i| list.remove(i))
}

fn compare_peer_id_topology_distance(compare: &PeerKey, left: &PeerKey, right: &PeerKey) -> i8 {
    for i in 0..compare.len() {
        let ds1 = calculate_one_byte_topology_distance(compare[i], left[i]);
        let ds2 = calculate_one_byte_topology_distance(compare[i], right[i]);
        if ds1 < ds2 {
            return 1;
        } else if ds1 > ds2 {
            return -1;
        }
    }
    0
}

pub fn calculate_one_byte_topology_distance(dst: u8, src: u8) -> u8 {
    let mut disnum = 0;
    if dst > src {
        disnum = dst - src
    } else if dst < src {
        disnum = src - dst
    }
    if disnum > 128 {
        disnum = 128 - (disnum - 128);
    }
    disnum
}
