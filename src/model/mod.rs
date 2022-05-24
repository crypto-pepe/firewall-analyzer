use std::collections::HashMap;

use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Clone, PartialEq)]
pub struct Request {
    pub remote_ip: String,
    pub host: String,
    pub method: String,
    pub path: String,
    pub headers: HashMap<String, String>,
    pub body: String,
}

#[derive(Debug, Serialize)]
pub struct BanRequest {
    pub target: BanTarget,
    pub reason: String,
    pub ttl: u32,
    #[serde(skip_serializing)]
    pub analyzer: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "kebab-case")]
pub struct BanTarget {
    pub ip: Option<String>,
    pub user_agent: Option<String>,
}
