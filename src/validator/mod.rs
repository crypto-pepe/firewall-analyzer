use serde::{Deserialize, Serialize};

use crate::model;
use crate::model::Request;
use crate::validator::dummy::Dummy as DummyValidator;
use crate::validator::generic_validator::count::CostCount;
use crate::validator::generic_validator::rule::BanRuleConfig;
use crate::validator::ip_count::IPReqCountValidator;

pub mod dummy;
mod generic_validator;
mod ip_count;
pub mod service;
mod request_cost;

pub trait Validator {
    fn validate(&mut self, req: Request) -> Result<Option<model::BanRequest>, anyhow::Error>;
    fn name(&self) -> String;
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Config {
    Dummy(dummy::Config),
    IpCount {
        limits: Vec<BanRuleConfig>,
        ban_description: String,
    },
}

pub fn get_validator(cfg: Config) -> Box<dyn Validator + Sync + Send> {
    match cfg {
        Config::Dummy(cfg) => Box::new(DummyValidator::new(cfg)),
        Config::IpCount {
            limits: rules,
            ban_description,
        } => Box::new(IPReqCountValidator::new(
            rules,
            CostCount {},
            ban_description,
            "ip_count".to_string(),
        )),
    }
}
