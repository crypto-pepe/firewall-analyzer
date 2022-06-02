use std::collections::HashMap;
use chrono::{DateTime, NaiveDateTime, Utc};
use circular_queue::CircularQueue;
pub(crate) use rule::BanRule;
pub use rule::BanRuleConfig;
pub use validator::CustomCostValidator;
use crate::model::{BanRequest, BanTarget};

mod rule;
mod state;
pub mod validator;

pub trait Cost:Ord {
    fn cost(&self) -> u64;
}

pub trait CostThreshold<C: Cost> {
    fn add(&mut self, req: C, time: DateTime<Utc>);
    fn latest_value_added_at(&self) -> Option<DateTime<Utc>>;
    fn clear(&mut self);
}

struct CountThreshold<T> {
    reqs: CircularQueue<DateTime<Utc>>
}

impl CostThreshold<DateTime<Utc>> for CountThreshold<T> {
    // fn set_limit(&mut self, limit: u64) {
    //     let mut reqs = CircularQueue::with_capacity(limit as usize);
    //     self.reqs.iter().for_each(|r| {reqs.push(*r);});
    //     self.reqs = reqs
    // }

    fn add(&mut self, _req: T, time: DateTime<Utc>) {
        self.reqs.push(time);
    }

    fn is_above_limit(&self) -> bool {
        !self.reqs.is_full() ||
    }

    fn latest_value_added_at(&self) -> Option<DateTime<Utc>> {
        *self.reqs.iter().last().clone()
    }

    fn clear(&mut self) {
        self.reqs.clear();
    }
}

#[derive(Debug)]
pub(crate) struct State<T> {
    pub cost_since_last_ban: u64,
    pub applied_rule_id: Option<usize>,
    pub base_costs: dyn CostThreshold<T>,
    pub resets_at: DateTime<Utc>,
}

impl State<T> {
    pub fn new(ct: impl CostThreshold<T>) -> Self {
        State {
            cost_since_last_ban: 0,
            applied_rule_id: None,
            base_costs: ct,
            resets_at: DateTime::<Utc>::from_utc(NaiveDateTime::from_timestamp(0, 0), Utc),
        }
    }

    pub fn should_reset_timeout(&self) -> bool {
        self.resets_at <= Utc::now() && self.applied_rule_id.is_some()
    }

    pub fn reset(&mut self, last_request_time: DateTime<Utc>) {
        self.base_costs.add(last_request_time);
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


pub struct CustomCostValidator<T, S> {
    ban_desc: String,
    rules: Vec<BanRule>,
    target_data: HashMap<T, State<S>>,
}

impl CustomCostValidator<T, S> {
    pub fn new<T, S>(rules: Vec<BanRuleConfig>, ban_desc: String) -> Self {
        let ip_data: HashMap<T, State<S>> = HashMap::new();
        CustomCostValidator {
            rules: rules.iter().map(|b| (*b).into()).collect(),
            ban_desc,
            target_data: ip_data,
        }
    }

    #[tracing::instrument(skip(self, rule_idx))]
    fn ban<T: Into<BanTarget>>(&self, rule_idx: usize, target: T) -> BanRequest {
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


impl Validator for CustomCostValidator<T, S> {
    #[tracing::instrument(skip(self))]
    fn validate(&mut self, req: Request) -> Result<Option<BanRequest>, Error> {
        let target = T::from(req);
        let rule = self.rules.get(0).expect("at least one rule required");
        let mut state = self
            .target_data
            .entry(target.clone())
            .or_insert_with(|| State::new(rule.limit as usize));

        let now = Utc::now();

        // No ban now
        if state.applied_rule_id.is_none() {
            state.base_costs.add(1, now);
            if !state.base_costs.is_above_limit() {
                return Ok(None);
            }
            if state.base_costs.latest_value_added_at() <= now - rule.reset_duration {
                return Ok(None);
            }

            state.resets_at = Utc::now() + rule.reset_duration;
            state.base_costs.clear();
            state.cost_since_last_ban = 0;
            state.applied_rule_id = Some(0);

            let rule = self.rules[0];
            tracing::info!(
                action = "ban",
                ip = target.as_str(),
                limit = rule.limit,
                ttl = rule.ban_duration.num_seconds()
            );
            return Ok(Some(self.ban(0, target)));
        }

        // was banned

        if state.should_reset_timeout() {
            state.reset(now);
            tracing::info!(action = "reset", ip = target.as_str());
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
                ip = target.as_str(),
                limit = rule.limit,
                ttl = rule.ban_duration.num_seconds()
            );
            return Ok(Some(self.ban(rule_idx, target)));
        }

        Ok(None)
    }

    fn name(&self) -> String {
        "ip_count".into()
    }
}
