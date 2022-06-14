use serde::{Deserialize, Serialize};

use crate::validators::common::BanRuleConfig;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Config {
    pub patterns: Vec<RequestPatternConfig>,
    pub limits: Vec<BanRuleConfig>,
    pub ban_description: String,
    pub default_cost: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RequestPatternConfig {
    pub method: String,
    pub path_regex: String,
    pub body_regex: String,
    pub cost: u64,
}
