use crate::validator::ip_count::BanRule;
use chrono::{DateTime, NaiveDateTime, Utc};
use circular_queue::CircularQueue;

#[derive(Debug)]
pub(crate) struct State {
    pub requests_since_last_ban: u64,
    pub applied_rule_idx: Option<usize>,
    pub recent_requests: CircularQueue<DateTime<Utc>>,
    pub resets_at: DateTime<Utc>,
}

impl State {
    pub fn new(requests_limit: usize) -> Self {
        State {
            requests_since_last_ban: 0,
            applied_rule_idx: None,
            recent_requests: CircularQueue::with_capacity(requests_limit),
            resets_at: DateTime::<Utc>::from_utc(NaiveDateTime::from_timestamp(0, 0), Utc),
        }
    }

    pub fn should_reset_timeout(&self) -> bool {
        self.resets_at <= Utc::now() && self.applied_rule_idx.is_some()
    }

    pub fn reset(&mut self, last_request_time: DateTime<Utc>) {
        self.recent_requests.push(last_request_time);
        self.applied_rule_idx = None;
    }

    pub fn apply_rule_if_possible(
        &mut self,
        rules: &[BanRule],
        rule_idx: usize,
        last_request_time: DateTime<Utc>,
    ) -> bool {
        let rule = rules
            .get(rule_idx)
            .expect(&*format!("rule {} not found", rule_idx));
        if self.requests_since_last_ban >= rule.limit {
            self.resets_at = last_request_time + rule.reset_duration;
            self.requests_since_last_ban = 0;
            self.applied_rule_idx = Some(rule_idx + 1);
            return true;
        }
        false
    }
}
