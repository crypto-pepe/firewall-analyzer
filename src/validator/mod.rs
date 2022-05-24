use serde::{Deserialize, Serialize};

use crate::model;
use crate::model::Request;
use crate::validator::dummy::Dummy as DummyValidator;
use crate::validator::Config::Dummy;

pub mod dummy;
pub mod service;

pub trait Validator {
    fn validate(&self, req: Request) -> Result<Option<model::BanRequest>, anyhow::Error>;
    fn name(&self) -> String;
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Config {
    Dummy { idx: u16 },
}

pub fn get_validator(cfg: Config) -> impl Validator {
    match cfg {
        Dummy { idx } => DummyValidator { idx },
    }
}
