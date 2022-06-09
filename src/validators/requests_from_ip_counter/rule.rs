use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct BanRuleConfig {
    pub limit: u64,
    pub ban_duration: duration_string::DurationString,
    pub reset_duration: duration_string::DurationString,
}

#[derive(Copy, Clone)]
pub struct BanRule {
    pub limit: u64,
    pub ban_duration: chrono::Duration,
    pub reset_duration: chrono::Duration,
}

impl TryFrom<BanRuleConfig> for BanRule {
    type Error = anyhow::Error;

    fn try_from(brc: BanRuleConfig) -> Result<Self, Self::Error> {
        Ok(Self {
            limit: brc.limit,
            ban_duration: chrono::Duration::from_std(brc.ban_duration.into())?,
            reset_duration: chrono::Duration::from_std(brc.reset_duration.into())?,
        })
    }
}
