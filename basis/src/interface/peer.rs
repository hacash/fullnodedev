

#[allow(async_fn_in_trait)]
pub trait NPeer {
    fn send_msg_on_block(&self, ty: u16, body: Vec<u8>) -> Rerr {
        println!("Peer send msg: {} - {}", ty, body.hex());
        Ok(())
    }
}