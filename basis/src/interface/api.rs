use std::collections::HashMap;
use std::sync::{Arc, Mutex};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ApiMethod {
    Get,
    Post,
}

#[derive(Clone, Debug, Default)]
pub struct ApiRequest {
    pub query: HashMap<String, String>,
    pub headers: HashMap<String, String>,
    pub body: Vec<u8>,
}

impl ApiRequest {
    pub fn query(&self, key: &str) -> Option<&str> {
        self.query.get(key).map(|s| s.as_str())
    }

    pub fn query_u64(&self, key: &str, dv: u64) -> u64 {
        self.query(key)
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(dv)
    }

    pub fn query_usize(&self, key: &str, dv: usize) -> usize {
        self.query(key)
            .and_then(|v| v.parse::<usize>().ok())
            .unwrap_or(dv)
    }
}

#[derive(Clone, Debug)]
pub struct ApiResponse {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

impl ApiResponse {
    pub fn json(body: String) -> Self {
        Self {
            status: 200,
            headers: vec![("content-type".to_owned(), "application/json".to_owned())],
            body: body.into_bytes(),
        }
    }
}

#[derive(Clone)]
pub struct ApiExecCtx {
    pub engine: Arc<dyn Engine>,
    pub hnoder: Arc<dyn HNoder>,
    pub launch_time: u64,
    pub miner_worker_notice_count: Arc<Mutex<u64>>,
}

pub type ApiHandlerFn = fn(&ApiExecCtx, ApiRequest) -> ApiResponse;

#[derive(Clone)]
pub struct ApiRoute {
    pub method: ApiMethod,
    pub path: String,
    pub handler: ApiHandlerFn,
}

impl ApiRoute {
    pub fn get(path: &str, handler: ApiHandlerFn) -> Self {
        Self {
            method: ApiMethod::Get,
            path: path.to_owned(),
            handler,
        }
    }

    pub fn post(path: &str, handler: ApiHandlerFn) -> Self {
        Self {
            method: ApiMethod::Post,
            path: path.to_owned(),
            handler,
        }
    }
}

pub trait ApiService: Send + Sync {
    fn name(&self) -> &'static str {
        "api-service"
    }

    fn routes(&self) -> Vec<ApiRoute>;
}
