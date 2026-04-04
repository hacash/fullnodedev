
impl P2PManage {

    pub(crate) async fn server(&self) -> TcpListener {
        let port = self.cnf.listen;
        let listener = TcpListener::bind( format!("0.0.0.0:{}", port) ).await.unwrap();
        listener
    }

}
