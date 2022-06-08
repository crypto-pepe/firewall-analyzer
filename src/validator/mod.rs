use serde::{Deserialize, Serialize};

use crate::model;
use crate::model::Request;
use crate::validator::dummy::Dummy as DummyValidator;
use crate::validator::requests_from_ip_counter::RequestsFromIpCounter;

pub mod dummy;
pub mod requests_from_ip_counter;
pub mod service;

pub trait Validator {
    fn validate(&mut self, req: Request) -> Result<Option<model::BanRequest>, anyhow::Error>;
    fn name(&self) -> String;
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Config {
    Dummy(dummy::Config),
    IpCount {
        limits: Vec<requests_from_ip_counter::BanRuleConfig>,
        ban_description: String,
    },
}

pub fn get_validator(cfg: Config) -> Box<dyn Validator + Sync + Send> {
    match cfg {
        Config::Dummy(cfg) => Box::new(DummyValidator::new(cfg)),
        Config::IpCount {
            limits,
            ban_description,
        } => Box::new(RequestsFromIpCounter::new(limits, ban_description)),
    }
}
