use crate::model;
use crate::model::{BanTarget, Request};
use crate::validator::Validator;

// USE ONLY FOR TESTING
// Dummy prints request and if dummy's idx is odd - bans ip for self idx * minutes
pub struct Dummy {
    pub idx: u16,
    pub ban_ttl_secs: u64,
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
