use std::fmt::format;

use crate::model::Request;
use crate::validator::Validator;

// Dummy just prints request and returns empty string
pub struct Dummy {}

impl Validator for Dummy {
    fn validate(&self, req: Request) -> Result<String, anyhow::Error> {
        println!("{:?}", req);
        Ok(String::new())
    }
}