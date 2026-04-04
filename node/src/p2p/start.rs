
impl P2PManage {

    pub async fn start(this: Arc<P2PManage>, worker: Worker) -> Rerr {
        this.start_peer_table_loop();

        let p2p = this.clone();
        tokio::spawn(async move{
            asleep(0.25).await;
            let _ = crate::core::connect_stable_then_boot(&p2p).await;
        });

        if this.cnf.find_nodes {
            let p2p = this.clone();
            tokio::spawn(async move{
                asleep(15.0).await;
                p2p.find_nodes().await
            });
        }

        let _ = crate::core::event_loop(this, worker).await;
        Ok(())
    }

}
