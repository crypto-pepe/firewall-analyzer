use crate::forwarder::http_client;
use duration_string::DurationString;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Config {
    #[serde(flatten)]
    pub http_config: http_client::Config,
    pub retry_count: usize,
    pub retry_wait: DurationString,
}
