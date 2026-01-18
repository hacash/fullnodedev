#[allow(unused)]
#[derive(Clone)]
pub struct HttpServer {
    cnf: ServerConf,
    engine: Arc<dyn Engine>,
    hcshnd: Arc<dyn HNoder>,
    router: Arc<Mutex<Option<Router>>>,
}


impl Server for HttpServer {
    fn start(&self, worker: Worker) {
        self.do_start(worker)
    }
}


impl HttpServer {
    
    pub fn open(iniobj: &IniObj, hnd: Arc<dyn HNoder>, router: Router) -> Self {
        let cnf = ServerConf::new(iniobj);
        Self{
            cnf: cnf,
            engine: hnd.engine(),
            hcshnd: hnd,
            router: Mutex::new(Some(router)).into(),
        }
    }
    
    fn do_start(&self, worker: Worker) {
        if !self.cnf.enable {
            worker.end();
            return // disable
        }
        let rt = new_tokio_rt(self.cnf.multi_thread);
        // server listen loop
        rt.block_on(async move {
            server_listen(self, worker).await
        });
    }


}


async fn server_listen(ser: &HttpServer, worker: Worker) {
    let port = ser.cnf.listen;
    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    let listener = TcpListener::bind(addr).await;
    if let Err(ref e) = listener {
        println!("\n[Error] Api Server bind port {} error: {}\n", port, e);
        worker.end();
        return
    }
    let listener = listener.unwrap();
    println!("[Http Api Server] Listening on http://{addr}");
    /* let app = route(ApiCtx::new(
        ser.engine.clone(),
        ser.hcshnd.clone(),
    )); */
    let rtapp = ser.router.lock().unwrap().take().unwrap();
    let mut wkr = worker.clone();
    if let Err(e) = axum::serve(listener, rtapp)
        .with_graceful_shutdown(async move {
            let _ = wkr.wait().await;
        })
        .await {
        println!("{e}");
    }
    println!("[Http Server] serve end.");
    worker.end();
}


