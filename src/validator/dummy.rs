use crate::model;
use crate::model::{BanTarget, Request};
use crate::validator::Validator;

// Dummy prints request and if dummy's idx is odd - bans ip for self idx * minutes
pub struct Dummy {
    pub idx: u16,
}

impl Validator for Dummy {
    fn validate(&self, req: Request) -> Result<Option<model::BanRequest>, anyhow::Error> {
        println!("{}: {:?}", self.idx, req);
        if self.idx % 2 == 1 {
            return Ok(Some(model::BanRequest {
                target: BanTarget {
                    ip: Some(req.remote_ip),
                    user_agent: None,
                },
                reason: format!("Validator has {} id", self.idx),
                ttl: (self.idx * 60) as u32,
                analyzer: self.name(),
            }));
        }
        Ok(None)
    }

    fn name(&self) -> String {
        format!("Dummy {}", self.idx)
    }
}
