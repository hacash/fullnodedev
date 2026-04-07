
pub trait NodeP2PExtension: Send + Sync {
    fn on_connect(&self, _: Arc<dyn NPeer>, _: Arc<dyn Engine>, _: Arc<dyn TxPool>) -> Rerr {
        Ok(())
    }

    fn on_disconnect(&self, _: Arc<dyn NPeer>) {}

    fn on_message(
        &self,
        _: Arc<dyn NPeer>,
        _: Arc<dyn Engine>,
        _: Arc<dyn TxPool>,
        _: u16,
        _: Vec<u8>,
    ) -> Rerr {
        Ok(())
    }
}
