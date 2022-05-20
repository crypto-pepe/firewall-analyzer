mod dummy;

use crate::model::Request;

pub trait Validator {
    // todo What result?
    fn validate(&self, req: Request) -> Result<String, anyhow::Error>;
}

