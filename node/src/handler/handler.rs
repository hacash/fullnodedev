pub struct MsgHandler {
    pub(crate) engine: Arc<dyn Engine>,
    pub(crate) txpool: Arc<dyn TxPool>,
    pub(crate) p2pmng: StdMutex<Option<Box<dyn PeerManage>>>,

    blktx: Sender<BlockTxArrive>,
    blktxch: StdMutex<Option<Receiver<BlockTxArrive>>>,

    pub(crate) doing_sync: AtomicU64,
    pub(crate) sync_tracker: SyncTracker,
    pub(crate) knows: Knowledge,

    pub(crate) inserting: Arc<StdMutex<bool>>,
}

impl MsgHandler {

    pub fn new(engine: Arc<dyn Engine>, txpool: Arc<dyn TxPool>) -> MsgHandler {
        let (tx, rx): (Sender<BlockTxArrive>, Receiver<BlockTxArrive>) = mpsc::channel(4000);
        MsgHandler{
            engine: engine,
            txpool: txpool,
            p2pmng: None.into(),
            blktx: tx,
            blktxch: Some(rx).into(),
            doing_sync: AtomicU64::new(0),
            sync_tracker: SyncTracker::new(),
            knows: Knowledge::new(200),
            inserting: Arc::new(StdMutex::new(false)),
        }
    }

    pub fn switch_peer(&self, p: Arc<Peer>) -> Arc<Peer> {
        self.p2pmng.lock().unwrap().as_ref().unwrap().switch_peer(p)
    }

    pub fn set_p2p_mng(&self, mng: Box<dyn PeerManage>) {
        let mut mymng = self.p2pmng.lock().unwrap();
        *mymng = Some(mng);
    }

    pub async fn submit_transaction(&self, body: Vec<u8>) {
        let _ = self.blktx.send(BlockTxArrive::Tx(None, body)).await;
    }

    pub async fn submit_block(&self, body: Vec<u8>) {
        let _ = self.blktx.send(BlockTxArrive::Block(None, body)).await;
    }

    pub fn exit(&self) {
        let lk = self.inserting.lock().unwrap();
        drop(lk)
    }

}


impl MsgHandler {

    pub async fn on_connect(&self, peer: Arc<Peer>) {
        let _ = peer.send_msg(MSG_REQ_STATUS, vec![]).await;
        let eng = self.engine.clone();
        let txp = self.txpool.clone();
        if let Err(e) = self.engine.minter().p2p_on_connect(peer, eng, txp) {
            println!("minter p2p on connect error: {}", e)
        }
    }

    pub async fn on_disconnect(&self, peer: Arc<Peer>) {
        self.sync_tracker.clear_peer(&peer);
    }

    pub async fn on_message(&self, peer: Arc<Peer>, ty: u16, body: Vec<u8>) {
        match ty {
            MSG_TX_SUBMIT =>      { let _ = self.blktx.send(BlockTxArrive::Tx(Some(peer.clone()), body)).await; },
            MSG_BLOCK_DISCOVER => { let _ = self.blktx.send(BlockTxArrive::Block(Some(peer.clone()), body)).await; },
            MSG_BLOCK_HASH =>     { self.receive_hashs(peer, body).await; },
            MSG_REQ_BLOCK_HASH => { self.send_hashs(peer, body).await; },
            MSG_BLOCK =>          { self.receive_blocks(peer, body).await; },
            MSG_REQ_BLOCK =>      { self.send_blocks(peer, body).await; },
            MSG_REQ_STATUS =>     { self.send_status(peer).await; },
            MSG_STATUS =>         { self.receive_status(peer, body).await; },
            _ => (),
        };
    }


}
