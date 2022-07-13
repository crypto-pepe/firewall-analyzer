use std::collections::HashMap;

use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub enum Body {
    Original(String),
    Skipped,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct Request {
    pub timestamp: String,
    pub remote_ip: String,
    pub host: String,
    pub method: String,
    pub path: String,
    pub headers: HashMap<String, String>,
    pub body: Body,
}

#[derive(Debug, Serialize, Eq, PartialEq, Clone)]
pub struct BanRequest {
    pub target: BanTarget,
    pub reason: String,
    pub ttl: u32,
}

#[derive(Debug, Clone)]
pub struct ValidatorBanRequest {
    pub ban_request: BanRequest,
    pub validator_name: String,
}

#[derive(Debug, Serialize, Eq, PartialEq, Clone)]
pub struct BanTarget {
    pub ip: Option<String>,
    pub user_agent: Option<String>,
}
