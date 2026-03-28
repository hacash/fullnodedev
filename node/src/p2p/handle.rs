

impl P2PManage {

    async fn handle_peer_message(&self, peer: Arc<Peer>, conn_read: OwnedReadHalf) -> Rerr {

        let peer1 = peer.clone();
        let peer2 = peer.clone();
        let peer3 = peer.clone();
        let peersnaprx = self.peersnaprx.clone();
        let peertabletx = self.peertabletx.clone();
        let hdl1 = self.msghandler.clone();
        let hdl2 = self.msghandler.clone();
        let hdl3 = self.msghandler.clone();
        tokio::spawn(async move {
            // handle msg
            do_handle_pmsg(peersnaprx, peertabletx, hdl2, peer2, conn_read).await;
            // on disconnect
            let hdlcp = hdl3;
            tokio::spawn(async move {
                hdlcp.on_disconnect(peer3).await
            });
        });
        // on connect
        tokio::spawn(async move {
            // println!("&&&& hdl1.on_connect(peer1) ...");
            hdl1.on_connect(peer1).await;
            // println!("&&&& hdl1.on_connect(peer1) ok.");
        });
        Ok(())
    }

}

async fn do_handle_pmsg(peersnaprx: PeerSnapRx, peertabletx: PeerTableCmdTx, msghdl: Arc<MsgHandler>, 
    peer: Arc<Peer>, mut conn_read: OwnedReadHalf
) {
    {   // print connect tips
        let peersnap = peersnaprx.borrow().clone();
        println!("[Peer] {} connected, total {} public {} subnet.", 
            peer.nick(), peersnap.backbones.len(), peersnap.offshoots.len());
    }
    // run loop
    loop {
        let rdres = tokio::select! {
            _ = peer.close_notify.notified() => {
                break // locally requested close
            }
            rd = tcp_read_msg(&mut conn_read, 0) => rd, // no timeout
        };
        if let Err(_) = rdres {
            break // closed
        }
        peer.update_active();
        let (ty, msg) = rdres.unwrap();
        // msg handle
        if MSG_CUSTOMER == ty {
            // on customer message
            if msg.len() < 2 {
                continue // ignore invalid msg
            }
            let prcp = peer.clone();
            let ty = u16::from_be_bytes( bufcut!(msg,0,2) );
            let body = msg[2..].to_vec();
            let msghd1 = msghdl.clone();
            tokio::spawn(async move {
                msghd1.on_message(prcp, ty, body).await;
            });
            continue // next
        }else if MSG_PING == ty {
            // replay pong
            let _ = peer.send_p2p_msg(MSG_PONG, vec![]).await;
        }else if MSG_PONG == ty {
            // do nothing
        }else if MSG_CLOSE == ty {
            // close the connect
            break // close
        }else{
            // ignore
        }
        // println!("=== Peer {} msg {} === {}", peer.nick(), ty, hex::encode(msg));
        // next
    }
    // 
    // println!("--- drop the Peer {}", peer.nick());
    // close the conn
    peer.disconnect().await;
    // remove from list
    let _ = peertabletx.send(PeerTableCmd::Remove(peer.clone()));
}
