use crate::model::Request;
use crate::validator::generic_validator::{BanRule, BaseCostCounter};
use chrono::{DateTime, NaiveDateTime, Utc};

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
