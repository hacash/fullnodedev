
impl P2PManage {

    pub async fn start(this: Arc<P2PManage>, worker: Worker) -> Rerr {

        // connect boot nodes
        let p2p = this.clone();
        tokio::spawn(async move{
            asleep(0.25).await;
            let _ = p2p.connect_stable_then_boot().await;
        });

        // do once find nodes
        if this.cnf.find_nodes {
            let p2p = this.clone();
            tokio::spawn(async move{
                asleep(15.0).await;
                p2p.find_nodes().await
            });
        }

        let _ = P2PManage::event_loop(this, worker).await;
        Ok(())
    }


}
