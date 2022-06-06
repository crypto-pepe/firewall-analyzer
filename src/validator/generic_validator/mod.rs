use crate::model::Request;
use crate::validator::generic_validator::rule::BanRule;
use chrono::{DateTime, Utc};
use std::fmt::Debug;

pub mod count;
pub mod rule;
mod state;
mod validator;

pub use crate::validator::generic_validator::validator::CustomCostValidator;

pub trait BaseCostCounter: Debug {
    fn new(r: &BanRule) -> Self;
    fn add(&mut self, req: Request, time: DateTime<Utc>);
    fn latest_value_added_at(&self) -> Option<DateTime<Utc>>;
    fn is_above_limit(&self, time: &DateTime<Utc>) -> bool;
    fn clear(&mut self);
}

pub trait RequestCoster {
    fn cost(&self, r: &Request) -> u64;
}
