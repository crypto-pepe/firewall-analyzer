use pepe_config::DurationString;
use serde::{Deserialize, Serialize};

use crate::forwarder::http_client;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Config {
    #[serde(flatten)]
    pub http_config: http_client::Config,
    pub retry_count: usize,
    pub retry_interval: DurationString,
}
