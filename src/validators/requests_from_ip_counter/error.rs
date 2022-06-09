use thiserror::Error;

#[derive(Error, Debug)]
pub enum RulesError {
    #[error("rule {0} not found")]
    NotFound(usize),
}
