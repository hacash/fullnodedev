use axum::routing::*;

use basis::server::ApiCtx;


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
fn take_routers() -> Vec<SvrRouter> {
    unsafe {
        SERVER_ROUTE_LIST.take().unwrap_or_default()
    }
}


/*
    routers
*/
pub fn route(ctx: ApiCtx) -> Router {
    let mut rtr = Router::new()
        .route("/_server_", get("Hacash Api Server"));
    for r in take_routers() {
        rtr = rtr.merge(r);
    }
    rtr.with_state(ctx)
}
    