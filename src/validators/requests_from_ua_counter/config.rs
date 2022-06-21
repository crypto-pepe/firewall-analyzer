use serde::{Deserialize, Serialize};

use crate::validators::common::BanRuleConfig;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Config {
    pub limits: Vec<BanRuleConfig>,
    pub patterns: Vec<String>,
    pub ban_description: String,
}
