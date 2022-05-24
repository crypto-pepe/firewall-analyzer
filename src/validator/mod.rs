use crate::model;
use crate::model::Request;

pub mod dummy;
pub mod service;

pub trait Validator {
    fn validate(&self, req: Request) -> Result<Option<model::BanRequest>, anyhow::Error>;
    fn name(&self) -> String;
}
