use duration_string::DurationString;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Config {
    pub analyzer_name: String,
    pub retry_count: usize,
    pub retry_wait: DurationString,
}
