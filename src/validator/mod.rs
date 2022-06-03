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
    Dummy(dummy::Config),
}

pub fn get_validator(cfg: Config) -> Box<dyn Validator + Sync + Send> {
    Box::new(match cfg {
        Config::Dummy(cfg) => DummyValidator::new(cfg),
    })
}
