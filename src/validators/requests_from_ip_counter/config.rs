use crate::validators::requests_from_ip_counter;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Config {
    pub limits: Vec<requests_from_ip_counter::BanRuleConfig>,
    pub ban_description: String,
}
