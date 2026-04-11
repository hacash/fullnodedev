
impl P2PManage {

    pub(crate) async fn server(&self) -> std::io::Result<TcpListener> {
        let port = self.cnf.listen;
        TcpListener::bind(format!("0.0.0.0:{}", port)).await
    }

}
