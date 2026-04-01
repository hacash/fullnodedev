use tokio::sync::Notify;

static PEER_AUTO_ID_INCREASE: AtomicU64 = AtomicU64::new(0);


#[derive(Debug)]
pub struct Peer {
    pub id: u64,
    pub key: PeerKey,
    pub name: String,
    pub is_public: bool,
    pub is_cntome: bool,
    pub addr: SocketAddr,
    pub active: StdMutex<SystemTime>,
    pub conn_write: StdMutex<Option<OwnedWriteHalf>>,
    pub close_notify: Arc<Notify>,
    pub knows: Knowledge,
}

impl Peer {

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn nick(&self) -> String {
        let mut nick = self.name.clone();
        if self.is_public {
            nick += format!("<{}>", self.addr).as_str();
        }
        nick
    }

    pub fn update_active(&self) {
        *self.active.lock().unwrap() = SystemTime::now();
    }

    fn take_conn_write(&self) -> Option<OwnedWriteHalf> {
        self.conn_write.lock().unwrap().take()
    }

    pub async fn disconnect(&self) {
        self.close_notify.notify_one();
        let mayconn = self.take_conn_write();
        if let Some(mut w) = mayconn {
            let close_msg = tcp_create_msg(MSG_CLOSE, vec![]);
            let _ = tcp_send(&mut w, &close_msg).await;
        }
    }

    pub async fn create_with_msg(mut stream: TcpStream, ty: u8, msg: Vec<u8>, mynodeinfo: Vec<u8>) -> Ret<(Arc<Peer>, OwnedReadHalf)> {
        let mut mykeyname = mynodeinfo;
        if mykeyname.len() > PEER_KEY_SIZE*2 {
            mykeyname = mykeyname[4..].to_vec();
        }
        let conn  = &mut stream;
        let mut addr = conn.peer_addr().unwrap();
        let mut is_public = false;
        let mut is_cntome = false;
        let idnamebts: &[u8];
        let mut oginport: u16 = 0;
        if msg.len() < 4 {
            return errf!("msg length too short (min 4)")
        }
        if MSG_REPORT_PEER == ty {
            is_cntome = true;
            oginport = u16::from_be_bytes( bufcut!(msg, 2, 4) );
            idnamebts = &msg[4..];
        }else if MSG_ANSWER_PEER == ty {
            is_public = !addr.ip().is_loopback();
            idnamebts = &msg[..];
        }else{
            return errf!("unsupported msg type {}", ty)
        }
        if idnamebts.len() < 32 {
            return errf!("msg length too short (min 32)")
        }
        let peerkey = bufcut!(idnamebts, 0, PEER_KEY_SIZE);
        let name = Fixed16::from( bufcut!(idnamebts, PEER_KEY_SIZE, PEER_KEY_SIZE*2) ).to_readable().replace(" ", "");
        if peerkey == mykeyname[0..PEER_KEY_SIZE] {
            return  errf!("cannot connect to self")
        }
        if MSG_REPORT_PEER == ty {
            tcp_send_msg(conn, MSG_ANSWER_PEER, mykeyname.clone()).await?;
            if oginport > 0 {
                let mut pubaddr = addr.clone();
                pubaddr.set_port(oginport);
                if let Ok(pb) = tcp_dial_to_check_is_public_id(pubaddr, &peerkey, 3).await {
                    if pb && !addr.ip().is_loopback() {
                        is_public = true;
                        addr.set_port(oginport);
                    }
                }
            }
        }

        let (read_half, write_half) = stream.into_split();

        let atid = PEER_AUTO_ID_INCREASE.fetch_add(1, Ordering::Relaxed) + 1;

        let peer = Peer {
            id: atid,
            key: peerkey,
            name: name,
            is_cntome: is_cntome,
            is_public: is_public,
            addr: addr,
            active: SystemTime::now().into(),
            conn_write: Some(write_half).into(),
            close_notify: Arc::new(Notify::new()),
            knows: Knowledge::new(50),
        };
        let pptr = Arc::new(peer);

        Ok((pptr, read_half))
    }


}
