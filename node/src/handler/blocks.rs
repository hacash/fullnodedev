

impl MsgHandler {

    async fn send_blocks(&self, peer: Arc<Peer>, buf: Vec<u8>) {
        if buf.len() != 8 {
            return // error len
        }
        let starthei = u64::from_be_bytes( bufcut!(buf, 0, 8) );
        let latest = self.engine.latest_block();
        let lathei = latest.height().uint();
	    let maxsendsize = 1024 * 1024 * 20usize; // max 20MB
	    let maxsendnum = 10000usize; // max 10000
        let mut totalsize = 0;
        let mut totalnum = 0;
        let mut endhei = 0;
        // load data
        let store = self.engine.store();
        let mut blkdtsary = vec![];
        for hei in starthei ..= lathei {
            let Some((_, blkdts)) = store.block_data_by_height(&BlockHeight::from(hei)) else {
                return // not find block hash by height
            };
            totalsize += blkdts.len();
            totalnum += 1;
            endhei = hei;
            blkdtsary.push( blkdts );
            if totalnum >= maxsendnum || totalsize >= maxsendsize {
                break // chunk finish
            }
        }
        let resblkdts = blkdtsary.concat();
        // ret
        let msgbody = vec![
            lathei.to_be_bytes().to_vec(),
            starthei.to_be_bytes().to_vec(),
            endhei.to_be_bytes().to_vec(),
            resblkdts,
        ].concat();
        // return blocks to peer
        let _ = peer.send_msg(MSG_BLOCK, msgbody).await;
    }
    
    async fn receive_blocks(&self, peer: Arc<Peer>, mut buf: Vec<u8>) {
        // println!("&&&& receive_blocks ...");
        if buf.len() < 3 * 8 {
            println!("check data failed.");
            return
        }
        let blocks = buf.split_off(3*8);
        let latest_hei = u64::from_be_bytes( bufcut!(buf, 0, 8) );
        let _start_hei = u64::from_be_bytes( bufcut!(buf, 8, 16) );
        let end_hei = u64::from_be_bytes( bufcut!(buf, 16, 24) );
        let persent =  end_hei as f64 / latest_hei as f64 * 100.0;
        // println!("&&&& receive_blocks start_hei={}, latest_hei={}, end_hei={} ...", start_hei, latest_hei, end_hei);
        {
            let isrlk = self.inserting.lock().unwrap();
            flush!("{}({:.2}%) inserting...", end_hei, persent);
            /* // let engine = self.engine.clone();
            let res = tokio::task::spawn_blocking(move || {
                engine.insert_sync(start_hei, blocks)
            }).await.unwrap(); */
            // try insert
            // let res = self.engine.insert_sync(start_hei blocks);
            let res = self.engine.synchronize(blocks);
            if let Err(e) = res {
                println!("{}", e); // show err
                return
            }
            println!("ok.");
            if end_hei >= latest_hei {
                println!("all blocks sync finished.");
                return
            }
            drop(isrlk);
        }
        // sync more blocks
        {
            let peer = self.switch_peer(peer);
            send_req_block_msg(self, peer, end_hei+1).await;
        }

    }



}
