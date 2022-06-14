use serde::{Deserialize, Serialize};

use crate::model;
use crate::model::Request;
use crate::validators::requests_from_ip_cost;
use crate::validators::{Dummy, RequestsFromIPCost};
use crate::validators::{dummy, requests_from_ip_counter, RequestsFromIPCounter};
pub mod service;
use anyhow::Result;

pub trait Validator {
    fn validate(&mut self, req: Request) -> Result<Option<model::BanRequest>>;
    fn name(&self) -> String;
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Config {
    Dummy(dummy::Config),
    #[serde(rename = "requests_from_ip_counter")]
    RequestsFromIPCounter(requests_from_ip_counter::Config),
    RequestsFromIPCost(requests_from_ip_cost::Config),
}

pub fn get_validator(cfg: Config) -> Result<Box<dyn Validator + Sync + Send>> {
    Ok(match cfg {
        Config::Dummy(cfg) => Box::new(Dummy::new(cfg)?),
        Config::RequestsFromIPCounter(cfg) => Box::new(RequestsFromIPCounter::new(cfg)?),
        Config::RequestsFromIPCost(cfg) => Box::new(RequestsFromIPCost::new(cfg)?),
    })
}
