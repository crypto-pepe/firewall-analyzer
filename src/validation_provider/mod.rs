use serde::{Deserialize, Serialize};

use crate::model;
use crate::model::Request;
use crate::validators::Dummy;
use crate::validators::{dummy, requests_from_ip_counter, RequestsFromIPCounter};
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

pub fn get_validator(cfg: Config) -> anyhow::Result<Box<dyn Validator + Sync + Send>> {
    Ok(match cfg {
        Config::Dummy(cfg) => Box::new(Dummy::new(cfg)?),
        Config::RequestsFromIPCounter(cfg) => Box::new(RequestsFromIPCounter::new(cfg)?),
    })
}
