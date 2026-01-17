
use axum::routing::*;

use super::server::*;


pub type SvrRouter = Router<ApiCtx>;

static mut SERVER_ROUTE_LIST: OnceLock<Vec<SvrRouter>> = OnceLock::new();

#[allow(static_mut_refs)]
pub fn server_router(router: SvrRouter) {
    unsafe {
        SERVER_ROUTE_LIST.get_or_init(||vec![]);
        let list = SERVER_ROUTE_LIST.get_mut().unwrap();
        list.push(router);
    }
}


#[allow(static_mut_refs)]
pub fn routers() -> Vec<SvrRouter> {
    unsafe {
        SERVER_ROUTE_LIST.take().unwrap_or_default()
    }
}

