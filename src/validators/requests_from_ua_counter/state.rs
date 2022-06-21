use chrono::{DateTime, Utc};
use circular_queue::CircularQueue;

use crate::validators::common::AppliedRule;

#[derive(Debug)]
pub(crate) struct State {
    pub requests_since_last_ban: u64,
    pub applied_rule: Option<AppliedRule>,
    pub recent_requests: CircularQueue<DateTime<Utc>>,
}

impl State {
    pub fn new(requests_limit: usize) -> Self {
        Self {
            requests_since_last_ban: 0,
            applied_rule: None,
            recent_requests: CircularQueue::with_capacity(requests_limit),
        }
    }

    pub fn should_reset_timeout(&self, by_time: DateTime<Utc>) -> bool {
        match self.applied_rule {
            None => false,
            Some(AppliedRule { resets_at, .. }) => resets_at <= by_time,
        }
    }

    pub fn reset(&mut self, last_request_time: DateTime<Utc>) {
        self.recent_requests.push(last_request_time);
        self.applied_rule = None;
    }

    pub fn push(&mut self, time: DateTime<Utc>) {
        self.recent_requests.push(time);
    }

    pub fn is_above_limit(&self, by_time: DateTime<Utc>) -> bool {
        if !self.recent_requests.is_full() {
            return false;
        }
        *self.recent_requests.iter().last().unwrap() <= by_time
    }

    pub fn clear(&mut self) {
        self.recent_requests.clear();
        self.requests_since_last_ban = 0;
    }

    pub fn apply_rule(&mut self, rule_idx: usize, resets_at: DateTime<Utc>) {
        self.applied_rule = Some(AppliedRule {
            rule_idx,
            resets_at,
        });
    }
}
