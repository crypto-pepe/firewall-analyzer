use std::cmp::min;
use std::collections::HashMap;
use std::fmt::Debug;
use std::hash::Hash;

use crate::validator::generic_validator::state::State;
use anyhow::Error;
use chrono::Utc;

use crate::model::{BanRequest, BanTarget, Request};
use crate::validator::generic_validator::rule::BanRuleConfig;
use crate::validator::generic_validator::{BanRule, BaseCostCounter, RequestCoster};
use crate::validator::Validator;

pub struct CustomCostValidator<
    T: From<Request> + Hash + Eq + Into<BanTarget>,
    S: BaseCostCounter,
    C: RequestCoster,
> {
    pub(crate) ban_desc: String,
    pub(crate) rules: Vec<BanRule>,
    pub(crate) target_data: HashMap<T, State<S>>,
    pub(crate) name: String,
    pub(crate) coster: C,
}

impl<
        T: From<Request> + Hash + Eq + Clone + Into<BanTarget> + Debug,
        S: BaseCostCounter,
        C: RequestCoster,
    > CustomCostValidator<T, S, C>
{
    pub fn new(rules: Vec<BanRuleConfig>, coster: C, ban_desc: String, name: String) -> Self {
        let ip_data: HashMap<T, State<S>> = HashMap::new();
        CustomCostValidator {
            rules: rules.iter().map(|b| (*b).into()).collect(),
            ban_desc,
            name,
            coster,
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

        state.cost_since_last_ban += self.coster.cost(&req);

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
