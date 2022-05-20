use std::collections::HashMap;

use serde::Deserialize;

#[derive(Debug, Deserialize, Clone, PartialEq)]
pub struct Request {
    pub remote_ip: String,
    // pub host: String,
    // pub method: String,
    // pub path: String,
    // pub headers: HashMap<String, String>,
    // pub body: String,
}