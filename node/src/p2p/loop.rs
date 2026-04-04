
impl P2PManage {

    pub async fn event_loop(this: Arc<P2PManage>, worker: Worker) -> Rerr {
        crate::core::event_loop(this, worker).await
    }

}
