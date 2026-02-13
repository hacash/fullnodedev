use std::sync::Arc;

use basis::interface::ApiService;

pub fn api_servicer(services: Vec<Arc<dyn ApiService>>) {
    crate::set_api_services(services);
}
