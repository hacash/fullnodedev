
impl P2PManage {

    pub async fn event_loop(this: Arc<P2PManage>, mut worker: Worker) -> Rerr {
        let mut printpeer_tkr = new_ticker(60*97).await;
        let mut reconnect_tkr = new_ticker(51*33).await;
        let mut findnodes_tkr = new_ticker(52*60*4).await;
        let mut checkpeer_tkr = new_ticker(53*3).await;
        let mut boostndes_tkr = new_ticker(54*5).await;

        let server_listener = this.server().await;

        loop {
            tokio::select! {
                _ = worker.wait() => {
                    break
                }
                _ = printpeer_tkr.tick() => {
                    this.print_conn_peers();
                },
                _ = reconnect_tkr.tick() => {
                    let no_nodes = this.backbones().len() < 2;
                    if no_nodes && this.cnf.find_nodes {
                        let _ = this.connect_stable_then_boot().await;
                    }
                },
                _ = findnodes_tkr.tick() => {
                    if this.cnf.find_nodes {
                        this.find_nodes().await;
                    }
                },
                _ = checkpeer_tkr.tick() => {
                    this.check_active_nodes().await;
                    this.ping_nodes().await;
                },
                _ = boostndes_tkr.tick() => {
                    this.boost_public().await;
                    if this.backbones().len() == 0 {
                        let _ = this.connect_stable_then_boot().await;
                    }
                },
                client = server_listener.accept() => {
                    let Ok((client, _)) = terrunbox!( client ) else {
                        continue
                    };
                    if !this.cnf.accept_nodes {
                        continue
                    }
                    let tobj = this.clone();
                    tokio::spawn(async move {
                        tobj.handle_conn(client, false).await
                    });
                },
                else => break
            }
        }
        this.disconnect_all_peers().await;
        println!("[P2P] Event loop end.");
        Ok(())
    }

}
