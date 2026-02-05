

pub trait NPeer: Sync + Send {
    fn send_msg_on_block(&self, _ty: u16, _body: Vec<u8>) -> Rerr {
        // println!("Peer send msg: {} - {}", ty, sys::ToHex::to_hex(&body));
        Ok(())
    }
}