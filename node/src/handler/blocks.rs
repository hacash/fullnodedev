
impl MsgHandler {

    async fn send_blocks(&self, peer: Arc<Peer>, buf: Vec<u8>) {
        if buf.len() != 8 {
            return
        }
        let starthei = u64::from_be_bytes( bufcut!(buf, 0, 8) );
        let latest = self.engine.latest_block();
        let lathei = latest.height().uint();
	    let maxsendsize = 1024 * 1024 * 20usize;
	    let maxsendnum = 10000usize;
        let mut totalsize = 0;
        let mut totalnum = 0;
        let mut endhei = 0;
        let store = self.engine.store();
        let mut blkdtsary = vec![];
        for hei in starthei ..= lathei {
            let Some((_, blkdts)) = store.block_data_by_height(&BlockHeight::from(hei)) else {
                return
            };
            totalsize += blkdts.len();
            totalnum += 1;
            endhei = hei;
            blkdtsary.push( blkdts );
            if totalnum >= maxsendnum || totalsize >= maxsendsize {
                break
            }
        }
        let resblkdts = blkdtsary.concat();
        let msgbody = vec![
            lathei.to_be_bytes().to_vec(),
            starthei.to_be_bytes().to_vec(),
            endhei.to_be_bytes().to_vec(),
            resblkdts,
        ].concat();
        let _ = peer.send_msg(MSG_BLOCK, msgbody).await;
    }

    async fn receive_blocks(&self, peer: Arc<Peer>, mut buf: Vec<u8>) {
        if buf.len() < 3 * 8 {
            println!("data check failed");
            return
        }
        let blocks = buf.split_off(3*8);
        let latest_hei = u64::from_be_bytes( bufcut!(buf, 0, 8) );
        let _start_hei = u64::from_be_bytes( bufcut!(buf, 8, 16) );
        let end_hei = u64::from_be_bytes( bufcut!(buf, 16, 24) );
        let persent =  end_hei as f64 / latest_hei as f64 * 100.0;
        {
            let isrlk = self.inserting.lock().unwrap();
            flush!("{}({:.2}%) inserting...", end_hei, persent);
            let res = self.engine.synchronize(blocks);
            if let Err(e) = res {
                println!("{}", e);
                return
            }
            println!("ok.");
            if end_hei >= latest_hei {
                println!("all blocks sync finished.");
                return
            }
            drop(isrlk);
        }
        {
            let peer = self.switch_peer(peer);
            send_req_block_msg(self, peer, end_hei+1).await;
        }
    }

}
