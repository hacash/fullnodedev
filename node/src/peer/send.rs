use basis::interface::NPeer;
use crate::new_current_thread_tokio_rt;

impl NPeer for Peer {
    fn send_msg_on_block(&self, ty: u16, body: Vec<u8>) -> Rerr {
        new_current_thread_tokio_rt().block_on(async move {
            self.send_msg(ty, body).await
        })
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
        if self.is_writer_closed() {
            return errf!("peer may be closed")
        }
        match self.writer_tx.try_send(PeerWriterCmd::Send(buf.clone())) {
            Ok(_) => Ok(()),
            Err(tokio::sync::mpsc::error::TrySendError::Full(_)) => errf!("peer writer queue full"),
            Err(tokio::sync::mpsc::error::TrySendError::Closed(_)) => {
                self.mark_writer_closed();
                errf!("peer may be closed")
            }
        }
    }

}
