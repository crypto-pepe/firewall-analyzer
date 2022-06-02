use std::time::Duration;

use pepe_config::DurationString;
use serde::{Deserialize, Serialize};

use crate::model;
use crate::model::Request;
use crate::validator::dummy::Dummy as DummyValidator;
use crate::validator::Config::Dummy;

pub mod dummy;
mod generic_validator;
pub mod service;

pub trait Validator {
    fn validate(&mut self, req: Request) -> Result<Option<model::BanRequest>, anyhow::Error>;
    fn name(&self) -> String;
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Config {
    Dummy {
        idx: u16,
        ban_ttl: Option<DurationString>,
    },
}

pub fn get_validator(cfg: Config) -> Box<dyn Validator> {
    match cfg {
        Dummy { idx, ban_ttl } => {
            let ban_ttl_secs = match ban_ttl {
                Some(ban_ttl) => {
                    let dur: Duration = ban_ttl.into();
                    dur.as_secs()
                }
                None => 120,
            };

            Box::new(DummyValidator { idx, ban_ttl_secs })
        }
    }
}
