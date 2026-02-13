use axum::body::Bytes;
use axum::extract::{Query, State};
use axum::http::{HeaderName, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use std::sync::OnceLock;
use std::str::FromStr;

static GLOBAL_API_SERVICES: OnceLock<Mutex<Vec<Arc<dyn ApiService>>>> = OnceLock::new();

fn global_services_store() -> &'static Mutex<Vec<Arc<dyn ApiService>>> {
    GLOBAL_API_SERVICES.get_or_init(|| Mutex::new(vec![]))
}

pub(crate) fn set_api_services(services: Vec<Arc<dyn ApiService>>) {
    let mut locker = global_services_store().lock().unwrap();
    *locker = services;
}

pub fn global_api_services() -> Vec<Arc<dyn ApiService>> {
    global_services_store().lock().unwrap().clone()
}

fn api_exec_ctx(ctx: &ApiCtx) -> ApiExecCtx {
    ApiExecCtx {
        engine: ctx.engine.clone(),
        hnoder: ctx.hcshnd.clone(),
        launch_time: ctx.launch_time,
        miner_worker_notice_count: ctx.miner_worker_notice_count.clone(),
    }
}

fn map_headers(headers: &HeaderMap) -> HashMap<String, String> {
    headers
        .iter()
        .filter_map(|(k, v)| v.to_str().ok().map(|vv| (k.to_string(), vv.to_owned())))
        .collect()
}

fn api_resp_to_axum(resp: ApiResponse) -> Response {
    let status = StatusCode::from_u16(resp.status).unwrap_or(StatusCode::OK);
    let mut headers = HeaderMap::new();
    for (k, v) in resp.headers {
        let Ok(name) = HeaderName::from_str(&k) else {
            continue;
        };
        let Ok(value) = HeaderValue::from_str(&v) else {
            continue;
        };
        headers.insert(name, value);
    }
    (status, headers, resp.body).into_response()
}

fn route_handler_exec(
    handler: ApiHandlerFn,
    ctx: &ApiCtx,
    headers: HeaderMap,
    query: HashMap<String, String>,
    body: Vec<u8>,
) -> Response {
    let req = ApiRequest {
        query,
        headers: map_headers(&headers),
        body,
    };
    let exec = api_exec_ctx(ctx);
    api_resp_to_axum(handler(&exec, req))
}

fn build_method_router(route: ApiRoute) -> MethodRouter<ApiCtx> {
    match route.method {
        ApiMethod::Get => {
            let handler = route.handler;
            get(
                move |State(ctx): State<ApiCtx>,
                      headers: HeaderMap,
                      Query(query): Query<HashMap<String, String>>| async move {
                    route_handler_exec(handler, &ctx, headers, query, vec![])
                },
            )
        }
        ApiMethod::Post => {
            let handler = route.handler;
            post(
                move |State(ctx): State<ApiCtx>,
                      headers: HeaderMap,
                      Query(query): Query<HashMap<String, String>>,
                      body: Bytes| async move {
                    route_handler_exec(handler, &ctx, headers, query, body.to_vec())
                },
            )
        }
    }
}

pub fn merge_registered_services(
    mut rtr: Router<ApiCtx>,
    services: Vec<Arc<dyn ApiService>>,
) -> Router<ApiCtx> {
    let mut all_services = global_api_services();
    all_services.extend(services);
    for svc in all_services {
        for route in svc.routes() {
            let mr = build_method_router(route.clone());
            rtr = rtr.route(route.path.as_str(), mr);
        }
    }
    rtr
}
