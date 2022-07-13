use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::model::{BanRequest, Request};
use crate::validators::{
    dummy, requests_from_ip_cost, requests_from_ip_counter, requests_from_ua_counter, Dummy,
    RequestsFromIPCost, RequestsFromIPCounter, RequestsFromUACounter,
};

pub mod service;

pub trait Validator {
    fn validate(&mut self, req: Request) -> Result<Option<BanRequest>>;
    fn name(&self) -> String;
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Config {
    Dummy(dummy::Config),
    #[serde(rename = "requests_from_ip_counter")]
    RequestsFromIPCounter(requests_from_ip_counter::Config),
    #[serde(rename = "requests_from_ua_counter")]
    RequestsFromUACounter(requests_from_ua_counter::Config),
    #[serde(rename = "requests_from_ip_cost")]
    RequestsFromIPCost(requests_from_ip_cost::Config),
}

pub fn get_validator(cfg: Config) -> Result<Box<dyn Validator + Sync + Send>> {
    Ok(match cfg {
        Config::Dummy(cfg) => Box::new(Dummy::new(cfg)?),
        Config::RequestsFromIPCounter(cfg) => Box::new(RequestsFromIPCounter::new(cfg)?),
        Config::RequestsFromUACounter(cfg) => Box::new(RequestsFromUACounter::new(cfg)?),
        Config::RequestsFromIPCost(cfg) => Box::new(RequestsFromIPCost::new(cfg)?),
    })
}
