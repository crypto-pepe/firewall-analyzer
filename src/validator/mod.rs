pub mod dummy;
pub mod service;

use crate::model;
use crate::model::Request;

pub trait Validator {
    fn validate(&self, req: Request) -> Result<Option<model::BanRequest>, anyhow::Error>;
}

