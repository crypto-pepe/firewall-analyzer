use serde::{Deserialize, Serialize};

use crate::model;
use crate::model::Request;
use crate::validator::dummy::Dummy as DummyValidator;

pub mod dummy;
pub mod ip_count;
pub mod service;

pub trait Validator {
    fn validate(&mut self, req: Request) -> Result<Option<model::BanRequest>, anyhow::Error>;
    fn name(&self) -> String;
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Config {
    Dummy(dummy::Config),
}

pub fn get_validator(cfg: Config) -> Box<dyn Validator> {
    match cfg {
        Config::Dummy(cfg) => DummyValidator::new(cfg),
        Config::IpCount {
            limits,
            ban_description,
        } => Box::new(IPCount::new(limits, ban_description)),

    }
}
