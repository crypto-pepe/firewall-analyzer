use std::collections::HashMap;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Config {
    pub kafka_brokers: Vec<String>,
    pub topics: Vec<String>,
    pub group: Option<String>,
    pub client_id: Option<String>,
    pub fetch_min_bytes: i32,
    pub fetch_max_wait_time_secs: u64,
    pub fetch_max_bytes_per_partition: i32,
}