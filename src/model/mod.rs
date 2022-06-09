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

#[derive(Debug, Serialize, Eq, PartialEq)]
pub struct ValidatorBanRequest {
    pub target: BanTarget,
    pub reason: String,
    pub ttl: u32,
}

#[derive(Debug, Serialize, Eq, PartialEq)]
pub struct BanRequest {
    #[serde(flatten)]
    pub validator_ban_request: ValidatorBanRequest,
    #[serde(skip_serializing)]
    pub analyzer: String,
}

#[derive(Debug, Serialize, Eq, PartialEq)]
pub struct BanTarget {
    pub ip: Option<String>,
    pub user_agent: Option<String>,
}
