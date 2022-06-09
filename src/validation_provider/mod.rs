use serde::{Deserialize, Serialize};

use crate::model;
use crate::model::Request;
use crate::validation_provider::dummy::Dummy as DummyValidator;
use crate::validation_provider::requests_from_ip_counter::RequestsFromIPCounter;

pub mod dummy;
pub mod requests_from_ip_counter;
pub mod service;

pub trait Validator {
    fn validate(&mut self, req: Request) -> anyhow::Result<Option<model::BanRequest>>;
    fn name(&self) -> String;
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Config {
    Dummy(dummy::Config),
    #[serde(rename = "requests_from_ip_counter")]
    RequestsFromIPCounter(requests_from_ip_counter::Config),
}

pub fn get_validator(cfg: Config) -> Box<dyn Validator + Sync + Send> {
    match cfg {
        Config::Dummy(cfg) => Box::new(DummyValidator::new(cfg)),
        Config::RequestsFromIPCounter(cfg) => Box::new(RequestsFromIPCounter::new(cfg)),
    }
}
