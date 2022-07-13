use crate::validators::common::BanRuleConfig;

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Config {
    pub limits: Vec<BanRuleConfig>,
    pub ban_description: String,
}
