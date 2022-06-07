pub(crate) use rule::BanRule;
pub use rule::BanRuleConfig;
use thiserror::Error;
pub use validator::IPCount;
mod rule;
mod state;
pub mod validator;

#[derive(Error, Debug)]
pub enum StateError {
    #[error("rule {0} not found")]
    NoRules(usize),
}
