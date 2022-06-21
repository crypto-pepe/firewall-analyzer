use crate::validators::common::AppliedRule;
use chrono::{DateTime, Utc};
use circular_queue::CircularQueue;

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
}
