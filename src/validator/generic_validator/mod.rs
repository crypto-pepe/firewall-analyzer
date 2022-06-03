use std::cmp::min;
use std::collections::HashMap;
use std::fmt::Debug;
use std::hash::Hash;

use anyhow::Error;
use chrono::{DateTime, NaiveDateTime, Utc};
use circular_queue::CircularQueue;
use num_traits::PrimInt;
use tracing::Value;

pub(crate) use rule::BanRule;
pub use rule::BanRuleConfig;

use crate::model::{BanRequest, BanTarget, Request};
use crate::validator::Validator;

mod rule;

pub trait InitStateHolder: Debug {
    fn new(r: &BanRule) -> Self;
    fn add(&mut self, req: Request, time: DateTime<Utc>);
    fn latest_value_added_at(&self) -> Option<DateTime<Utc>>;
    fn is_above_limit(&self, time: &DateTime<Utc>) -> bool;
    fn clear(&mut self);
}

#[derive(Debug)]
pub struct CountStateHolder {
    reqs: CircularQueue<DateTime<Utc>>,
}

impl InitStateHolder for CountStateHolder {
    fn new(r: &BanRule) -> Self {
        CountStateHolder {
            reqs: CircularQueue::with_capacity(r.limit as usize),
        }
    }

    fn add(&mut self, _req: Request, time: DateTime<Utc>) {
        self.reqs.push(time);
    }

    fn latest_value_added_at(&self) -> Option<DateTime<Utc>> {
        match self.reqs.iter().last().clone() {
            Some(s) => Some(*s),
            None => None,
        }
    }

    fn is_above_limit(&self, time: &DateTime<Utc>) -> bool {
        if !self.reqs.is_full() {
            return false;
        }
        self.reqs.iter().last().expect("reqs is full") > time
    }

    fn clear(&mut self) {
        self.reqs.clear();
    }
}

#[derive(Debug)]
pub struct State<T: InitStateHolder> {
    pub cost_since_last_ban: u64,
    pub applied_rule_id: Option<usize>,
    pub base_costs: T,
    pub resets_at: DateTime<Utc>,
}

impl<IST: InitStateHolder> State<IST> {
    pub fn new(br: &BanRule) -> Self {
        State {
            cost_since_last_ban: 0,
            applied_rule_id: None,
            base_costs: IST::new(br),
            resets_at: DateTime::<Utc>::from_utc(NaiveDateTime::from_timestamp(0, 0), Utc),
        }
    }

    pub fn should_reset_timeout(&self) -> bool {
        self.resets_at <= Utc::now() && self.applied_rule_id.is_some()
    }

    pub fn reset(&mut self, req: Request, last_request_time: DateTime<Utc>) {
        self.base_costs.add(req, last_request_time);
        self.applied_rule_id = None;
    }

    pub fn try_apply_rule(
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
            self.applied_rule_id = Some(rule_idx + 1);
            return true;
        }
        false
    }
}

#[derive(Hash, Eq, PartialEq, Clone, Debug)]
pub struct RequestIP {
    ip: String,
}

impl From<Request> for RequestIP {
    fn from(r: Request) -> Self {
        RequestIP { ip: r.remote_ip }
    }
}

impl From<RequestIP> for BanTarget {
    fn from(r: RequestIP) -> Self {
        BanTarget {
            ip: Some(r.ip),
            user_agent: None,
        }
    }
}

pub type IPReqCountValidator = CustomCostValidator<RequestIP, CountStateHolder>;

pub struct CustomCostValidator<T: From<Request> + Hash + Eq + Into<BanTarget>, S: InitStateHolder> {
    ban_desc: String,
    rules: Vec<BanRule>,
    target_data: HashMap<T, State<S>>,
}

impl<T: From<Request> + Hash + Eq + Clone + Into<BanTarget> + Debug, S: InitStateHolder>
    CustomCostValidator<T, S>
{
    pub fn new(rules: Vec<BanRuleConfig>, ban_desc: String) -> Self {
        let ip_data: HashMap<T, State<S>> = HashMap::new();
        CustomCostValidator {
            rules: rules.iter().map(|b| (*b).into()).collect(),
            ban_desc,
            target_data: ip_data,
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

impl<T: From<Request> + Hash + Eq + Clone + Into<BanTarget> + Debug, S: InitStateHolder> Validator
    for CustomCostValidator<T, S>
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
        if state.applied_rule_id.is_none() {
            state.base_costs.add(req.clone(), now);
            if !state.base_costs.is_above_limit(&now) {
                return Ok(None);
            }
            if let Some(lvaa) = state.base_costs.latest_value_added_at() {
                if lvaa <= now - rule.reset_duration {
                    return Ok(None);
                }
            }

            state.resets_at = Utc::now() + rule.reset_duration;
            state.base_costs.clear();
            state.cost_since_last_ban = 0;
            state.applied_rule_id = Some(0);

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
            state.reset(req.clone(), now);
            tracing::info!(action = "reset", target = ?target);
            return Ok(None);
        }

        state.cost_since_last_ban += 1;

        let rule_idx = state
            .applied_rule_id
            .map_or(0, |v| min(v + 1, self.rules.len() - 1));

        if state.try_apply_rule(&self.rules, rule_idx, now) {
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
        "generic_counter".into()
    }
}
