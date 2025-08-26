

#[derive(Clone)]
pub struct NodeConf {
    pub node_key: [u8; 16],
    pub node_name: String,
    pub listen: u16,
    pub findnodes: bool,
    pub acceptnodes: bool,
    pub boot_nodes: Vec<SocketAddr>,
    pub offshoot_peers: usize, // private IP
    pub backbone_peers: usize, // public IP
    
    pub multi_thread: bool,


}


impl NodeConf {

    
    pub fn new(ini: &IniObj) -> NodeConf {
        let sec = &ini_section(ini, "node");

        // node key
        let node_key = read_node_key(ini, &sec);

        // node name
        let nidhx = hex::encode(&node_key);
        let defnm: String = "hn".to_owned() + &nidhx[..8];
        let node_name = ini_must_maxlen(&sec, "name", &defnm, 16); // max len = 16
        // println!("node name = {}", node_name);

        // port
        let port = ini_must_u64(sec, "listen", 3337);
        if port<1001 || port>65535 {
            panic!("{}", exiterr!(1,"node listen port '{}' not support", port))
        }
        // off_find
        let find = ini_must_bool(sec, "not_find_nodes", false) == false;
        let accept = ini_must_bool(sec, "not_accept_nodes", false) == false;

        // boots
        let boots = ini_must(sec, "boots", "");
        let boots = boots.replace(" ", "");
        let mut ipts: Vec<SocketAddr> = Vec::new();
        if ! boots.is_empty() {
            let boots = boots.split(",");
            ipts = boots.map(
                |s|s.parse::<SocketAddr>().expect(exiterr!(1,"boot node ip port '{}' not support", &s))
            ).collect();
        }
        // println!("boot nodes: {:?}", ipts);

        // create config
        let mut cnf = NodeConf{
            node_key: node_key,
            node_name: node_name,
            listen: port as u16,
            findnodes: find,
            acceptnodes: accept,
            boot_nodes: ipts,
            // connect peers
            offshoot_peers: 200,
            backbone_peers: 4,
            multi_thread:  ini_must_bool(sec, "multi_thread", false),
        };

        cnf.offshoot_peers = ini_must_u64(sec, "offshoot_peers", 200) as usize;
        cnf.backbone_peers = ini_must_u64(sec, "backbone_peers", 4) as usize;

        // ok
        cnf
    }

}


/**
 * 
 */
fn read_node_key(ini: &IniObj, sec: &HashMap<String, Option<String>>) -> [u8; 16] {

    // node.id path
    let mut nidfp = get_mainnet_data_dir(ini);
    let kph =  std::path::absolute(nidfp.as_path());
    nidfp.push("node.id");
        
    // node id
    let mut node_key = [0u8; 16];
    let mut nidfile = OpenOptions::new().read(true).write(true).create(true).open(nidfp).expect("cannot open node info file.");
    // read
    let mut snid = String::new();
    nidfile.read_to_string(&mut snid).unwrap();
    // println!("read node id = {}", snid);
    if let Ok(nid) = hex::decode(&snid) {
        if nid.len() == 16 {
            node_key = nid.try_into().unwrap();
        }
    }
    if node_key[0] == 0 && node_key[15] == 0 {
        // get random node key
        let ndn = ini_must_maxlen(&sec, "name", "hx8888", 16); // max len = 16
        let sst = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap().as_nanos();
        let stuff = format!("{}-{}-{}", kph.unwrap().display(), ndn, sst);
        println!("build node key by: {}", stuff);
        node_key = sys::sha2(&stuff)[0..16].try_into().unwrap();
        nidfile.write_all(hex::encode(&node_key).as_bytes()).unwrap();
    }
    // let nidhx = hex::encode(&node_key);
    // println!("node id = {}", nidhx);
    node_key
}