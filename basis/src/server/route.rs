

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


async fn _server_(State(_ctx): State<ApiCtx>, _: Request) -> impl IntoResponse {
    (html_headers(), "Hacash Api Server")
}

