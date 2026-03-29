
impl NPeer for Peer {
    
    fn send_msg_on_block(&self, ty: u16, body: Vec<u8>) -> Rerr {
        let _ = new_current_thread_tokio_rt().block_on(async move {
            self.send_msg(ty, body).await
        });
        Ok(())
    }

}


impl Peer {

    pub async fn send_msg(&self, ty: u16, body: Vec<u8>) -> Rerr {
        let msg = vec![ty.to_be_bytes().to_vec(), body].concat();
        self.send_p2p_msg(MSG_CUSTOMER, msg).await
    }

    pub async fn send_p2p_msg(&self, ty: u8, body: Vec<u8>) -> Rerr {
        let msgbuf = tcp_create_msg(ty, body);
        self.send(&msgbuf).await
    }

    pub async fn send(&self, buf: &Vec<u8>) -> Rerr {
        let mut wlk = self.conn_write.lock().await;
        let Some(w) = wlk.as_mut() else {
            return errf!("peer may be closed")
        };
        let send_res = tcp_send(w, buf).await;
        if let Err(e) = send_res {
            *wlk = None;
            return Err(e)
        }
        Ok(())
    }


}
