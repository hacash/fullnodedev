

// paths
pub fn query(p: &str) -> String {
    "/query/".to_owned() + p
}

pub fn create(p: &str) -> String {
    "/create/".to_owned() + p
}

pub fn submit(p: &str) -> String {
    "/submit/".to_owned() + p
}

pub fn operate(p: &str) -> String {
    "/operate/".to_owned() + p
}

pub fn util(p: &str) -> String {
    "/util/".to_owned() + p
}



/*
    routers
*/
pub fn route(ctx: ApiCtx) -> Router {
    let mut rtr = Router::new();
    for r in super::setup::routers() {
        rtr = rtr.merge(r);
    }
    rtr.with_state(ctx)
}
    