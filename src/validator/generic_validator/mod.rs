use std::cmp::min;
use std::collections::HashMap;
use std::fmt::Debug;
use std::hash::Hash;
use std::marker::PhantomData;

use anyhow::Error;
use chrono::{DateTime, NaiveDateTime, Utc};

pub(crate) use rule::BanRule;
pub use rule::BanRuleConfig;

use crate::model::{BanRequest, BanTarget, Request};
use crate::validator::Validator;

pub mod count;
pub mod rule;

pub trait BaseCostCounter: Debug {
    fn new(r: &BanRule) -> Self;
    fn add(&mut self, req: Request, time: DateTime<Utc>);
    fn latest_value_added_at(&self) -> Option<DateTime<Utc>>;
    fn is_above_limit(&self, time: &DateTime<Utc>) -> bool;
    fn clear(&mut self);
}

pub trait RequestCoster {
    fn cost(r: &Request) -> u64;
}

#[derive(Debug)]
pub struct State<T: BaseCostCounter> {
    pub cost_since_last_ban: u64,
    pub applied_rule_idx: Option<usize>,
    pub base_costs: T,
    pub resets_at: DateTime<Utc>,
}

impl<T: BaseCostCounter> State<T> {
    pub fn new(br: &BanRule) -> Self {
        State {
            cost_since_last_ban: 0,
            applied_rule_idx: None,
            base_costs: T::new(br),
            resets_at: DateTime::<Utc>::from_utc(NaiveDateTime::from_timestamp(0, 0), Utc),
        }
    }

    pub fn should_reset_timeout(&self) -> bool {
        self.resets_at <= Utc::now() && self.applied_rule_idx.is_some()
    }

    pub fn reset(&mut self, req: Request, last_request_time: DateTime<Utc>) {
        self.base_costs.add(req, last_request_time);
        self.applied_rule_idx = None;
    }

    pub fn apply_rule_if_required(
        &mut self,
        rules: &[BanRule],
        rule_idx: usize,
        last_request_time: DateTime<Utc>,
    ) -> bool {
        let rule = rules
            .get(rule_idx)
            .expect(&*format!("rule {} not found", rule_idx));
        if self.cost_since_last_ban >= rule.limit {
            self.resets_at = last_request_time + rule.reset_duration;
            self.cost_since_last_ban = 0;
            self.applied_rule_idx = Some(rule_idx + 1);
            return true;
        }
        false
    }
}

pub struct CustomCostValidator<
    T: From<Request> + Hash + Eq + Into<BanTarget>,
    S: BaseCostCounter,
    C: RequestCoster,
> {
    pub(crate) ban_desc: String,
    pub(crate) rules: Vec<BanRule>,
    pub(crate) target_data: HashMap<T, State<S>>,
    pub(crate) name: String,
    pub(crate) _phantom_c: PhantomData<C>,
}

impl<
        T: From<Request> + Hash + Eq + Clone + Into<BanTarget> + Debug,
        S: BaseCostCounter,
        C: RequestCoster,
    > CustomCostValidator<T, S, C>
{
    pub fn new(rules: Vec<BanRuleConfig>, ban_desc: String, name: String) -> Self {
        let ip_data: HashMap<T, State<S>> = HashMap::new();
        CustomCostValidator {
            rules: rules.iter().map(|b| (*b).into()).collect(),
            ban_desc,
            name,
            target_data: ip_data,
            _phantom_c: PhantomData::default(),
        }
    }

    #[tracing::instrument(skip(self, rule_idx))]
    fn ban(&self, rule_idx: usize, target: T) -> BanRequest {
        BanRequest {
            target: target.into(),
            reason: self.ban_desc.clone(),
            ttl: self
                .rules
                .get(rule_idx)
                .expect(&*format!("rule {} not found", rule_idx))
                .ban_duration
                .num_seconds() as u32,
            analyzer: self.name(),
        }
    }
}

impl<
        T: From<Request> + Hash + Eq + Clone + Into<BanTarget> + Debug,
        S: BaseCostCounter,
        C: RequestCoster,
    > Validator for CustomCostValidator<T, S, C>
{
    #[tracing::instrument(skip(self))]
    fn validate(&mut self, req: Request) -> Result<Option<BanRequest>, Error> {
        let target = T::from(req.clone());
        let rule = self.rules.get(0).expect("at least one rule required");
        let mut state = self
            .target_data
            .entry(target.clone())
            .or_insert_with(|| State::new(rule));

        let now = Utc::now();

        // No ban now
        if state.applied_rule_idx.is_none() {
            state.base_costs.add(req, now);
            if !state
                .base_costs
                .is_above_limit(&(now - rule.reset_duration))
            {
                return Ok(None);
            }
            if let Some(t) = state.base_costs.latest_value_added_at() {
                if t <= now - rule.reset_duration {
                    return Ok(None);
                }
            }

            state.resets_at = Utc::now() + rule.reset_duration;
            state.base_costs.clear();
            state.cost_since_last_ban = 0;
            state.applied_rule_idx = Some(0);

            let rule = self.rules[0];
            tracing::info!(
                action = "ban",
                taget = ?target,
                limit = rule.limit,
                ttl = rule.ban_duration.num_seconds()
            );
            return Ok(Some(self.ban(0, target)));
        }

        // was banned

        if state.should_reset_timeout() {
            state.reset(req, now);
            tracing::info!(action = "reset", target = ?target);
            return Ok(None);
        }

        state.cost_since_last_ban += C::cost(&req);

        let rule_idx = state
            .applied_rule_idx
            .map_or(0, |v| min(v + 1, self.rules.len() - 1));

        if state.apply_rule_if_required(&self.rules, rule_idx, now) {
            let rule = self.rules[rule_idx];
            tracing::info!(
                action = "ban",
                target = ?target,
                limit = rule.limit,
                ttl = rule.ban_duration.num_seconds()
            );
            return Ok(Some(self.ban(rule_idx, target)));
        }

        Ok(None)
    }

    fn name(&self) -> String {
        self.name.clone()
    }
}
