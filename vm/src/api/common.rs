struct VmApiService {}

pub fn service() -> Arc<dyn ApiService> {
    Arc::new(VmApiService {})
}

impl ApiService for VmApiService {
    fn name(&self) -> &'static str {
        "vm"
    }

    fn routes(&self) -> Vec<ApiRoute> {
        routes()
    }
}

fn api_error(errmsg: &str) -> ApiResponse {
    ApiResponse::json(json!({"ret":1,"err":errmsg}).to_string())
}

fn api_data_raw(s: String) -> ApiResponse {
    ApiResponse::json(format!(r#"{{"ret":0,{}}}"#, s))
}

fn req_hex(s: &str) -> Ret<Vec<u8>> {
    hex::decode(s).map_err(|_| "hex format error".to_owned())
}

fn req_addr(s: &str) -> Ret<Address> {
    Address::from_readable(s).map_err(|e| format!("address {} format error: {}", s, e))
}



