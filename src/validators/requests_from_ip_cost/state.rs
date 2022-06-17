use chrono::{DateTime, Utc};

#[derive(Debug, Clone)]
pub struct AppliedRule {
    pub applied_rule_idx: usize,
    pub resets_at: DateTime<Utc>,
}

#[derive(Debug)]
pub(crate) struct State {
    pub cost_limit: u64,
    pub applied_rule: Option<AppliedRule>,
    pub cost_since_last_ban: u64,
    pub recent_requests: Vec<(u64, DateTime<Utc>)>,
}

impl State {
    pub fn new(cost_limit: u64) -> Self {
        Self {
            cost_limit,
            cost_since_last_ban: 0,
            applied_rule: None,
            recent_requests: vec![],
        }
    }

    pub fn should_reset_timeout(&self, by_time: DateTime<Utc>) -> bool {
        match self.applied_rule {
            None => false,
            Some(AppliedRule { resets_at, .. }) => resets_at <= by_time,
        }
    }

    pub fn is_limit_reached(&self, for_time: DateTime<Utc>) -> bool {
        self.recent_requests
            .iter()
            .filter(|(_cost, t)| *t > for_time)
            .fold(0u64, |c, (cost, _)| c + cost)
            >= self.cost_limit
    }

    pub fn clean_before(&mut self, before: DateTime<Utc>) {
        self.recent_requests.retain(|(_, t)| *t >= before)
    }
}
