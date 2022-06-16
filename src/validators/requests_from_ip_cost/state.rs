use chrono::{DateTime, NaiveDateTime, Utc};

#[derive(Debug)]
pub(crate) struct State {
    pub cost_limit: u64,
    pub cost_since_last_ban: u64,
    pub applied_rule_idx: Option<usize>,
    pub recent_requests: Vec<(u64, DateTime<Utc>)>,
    pub resets_at: DateTime<Utc>,
}

impl State {
    pub fn new(cost_limit: u64) -> Self {
        Self {
            cost_limit,
            cost_since_last_ban: 0,
            applied_rule_idx: None,
            recent_requests: vec![],
            resets_at: DateTime::<Utc>::from_utc(NaiveDateTime::from_timestamp(0, 0), Utc),
        }
    }

    pub fn should_reset_timeout(&self) -> bool {
        self.resets_at <= Utc::now() && self.applied_rule_idx.is_some()
    }

    pub fn reset(&mut self, cost: u64, last_request_time: DateTime<Utc>) {
        self.recent_requests.push((cost, last_request_time));
        self.applied_rule_idx = None;
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
