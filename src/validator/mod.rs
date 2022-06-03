use std::time::Duration;

use pepe_config::DurationString;
use serde::{Deserialize, Serialize};

use crate::model;
use crate::model::Request;
use crate::validator::dummy::Dummy as DummyValidator;
use crate::validator::Config::Dummy;
use crate::validator::generic_validator::{BanRuleConfig, IPReqCountValidator};

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
    Dummy(dummy::Config),
    IpCount(Vec<BanRuleConfig>),
}

pub fn get_validator(cfg: Config) -> Box<dyn Validator + Sync + Send> {
    match cfg {
        Config::Dummy(cfg) => Box::new(DummyValidator::new(cfg)),
        Config::IpCount(brcs) => Box::new(IPReqCountValidator::new(brcs,String::new()))
    }
}
