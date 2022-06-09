use crate::model;
use crate::model::{BanTarget, Request};
use crate::validation_provider::Validator;
use pepe_config::DurationString;
use serde::{Deserialize, Serialize};
use std::time::Duration;

// USE ONLY FOR TESTING
// Dummy prints request and if dummy's idx is odd - bans ip for ban_ttl_secs or 120s, if not stated
pub struct Dummy {
    pub idx: u16,
    pub ban_ttl_secs: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Config {
    pub idx: u16,
    pub ban_ttl: Option<DurationString>,
}

impl Dummy {
    pub fn new(cfg: Config) -> Self {
        Dummy {
            idx: cfg.idx,
            ban_ttl_secs: {
                let dur: Duration = cfg
                    .ban_ttl
                    .unwrap_or_else(|| DurationString::from(Duration::from_secs(120)))
                    .into();
                dur.as_secs()
            },
        }
    }
}
impl Validator for Dummy {
    #[tracing::instrument(skip(self))]
    fn validate(&mut self, req: Request) -> Result<Option<model::BanRequest>, anyhow::Error> {
        if self.idx % 2 == 1 {
            return Ok(Some(model::BanRequest {
                target: BanTarget {
                    ip: Some(req.remote_ip),
                    user_agent: None,
                },
                reason: format!("Validator has {} id", self.idx),
                ttl: self.ban_ttl_secs as u32,
                analyzer: self.name(),
            }));
        }
        Ok(None)
    }

    fn name(&self) -> String {
        format!("Dummy {}", self.idx)
    }
}
