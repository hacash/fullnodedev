
fn routes() -> Vec<ApiRoute> {
    vec![
        ApiRoute::get("/query/contract/sandboxcall", contract_sandbox_call),
        ApiRoute::get("/query/contract/logs", vm_logs_read),
        ApiRoute::get("/operate/contract/logs/delete", vm_logs_del),
    ]
}