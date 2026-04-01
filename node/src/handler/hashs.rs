
impl MsgHandler {

    async fn send_hashs(&self, peer: Arc<Peer>, buf: Vec<u8>) {
        if buf.len() != 1+8 {
            return
        }
        let hnum = buf[0] as u64;
        if hnum > 80 {
            return
        }
        let endhei = u64::from_be_bytes( bufcut!(buf, 1, 9) );
        let latest = self.engine.latest_block();
        let lathei = latest.height().uint();
        if endhei > lathei {
            return
        }
        let mut starthei = endhei - hnum;
        if hnum >= endhei {
            starthei = 1;
        }
        let store = self.engine.store();
        let mut reshxs = Vec::with_capacity((hnum + 8) as usize);
        reshxs.push( buf[1..9].to_vec() );
        for hei in (starthei..=endhei).rev() {
            let curhx = store.block_hash(&BlockHeight::from(hei));
            if curhx.is_none() {
                return
            }
            reshxs.push( curhx.unwrap().to_vec() );
        }
        let _ = peer.send_msg(MSG_BLOCK_HASH, reshxs.concat()).await;
    }

    async fn receive_hashs(&self, peer: Arc<Peer>, mut buf: Vec<u8>) {
        if buf.len() < 8 {
            return
        }
        let hashs = buf.split_off(8);
        let end_hei = u64::from_be_bytes( bufcut!(buf, 0, 8) );
        let hash_len = hashs.len();
        if hash_len == 0 || hash_len % 32 != 0 {
            return
        }
        let mut hash_num = hash_len as u64 / 32;
        let latest = self.engine.latest_block();
        let lathei = latest.height().uint();
        if end_hei > lathei {
            return
        }
        let dfhmax = self.engine.config().unstable_block as u64 + 1;
        if hash_num > dfhmax {
            hash_num = dfhmax;
        }
        let mut start_hei = end_hei - hash_num;
        if end_hei <= hash_num {
            start_hei = 0;
        }
        let store = self.engine.store();
        let mut hi = 0;
        for hei in ((start_hei+1)..=end_hei).rev() {
            let myhx = store.block_hash(&BlockHeight::from(hei));
            if myhx.is_none() {
                return
            }
            let myhx = myhx.unwrap();
            let hx = Fixed32::from( bufcut!(hashs, hi, hi+32) );
            if hx == myhx {
                get_status_try_sync_blocks(self, peer, hei + 1).await;
                return
            }
            hi += 32;
        }
    }

}
