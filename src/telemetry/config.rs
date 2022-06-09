use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Config {
    pub svc_name: String,
    pub jaeger_endpoint: Option<String>,
}
