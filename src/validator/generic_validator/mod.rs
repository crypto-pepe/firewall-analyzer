use std::cmp::min;
use std::collections::HashMap;
use std::fmt::Debug;
use std::hash::Hash;
use std::marker::PhantomData;

use anyhow::Error;
use chrono::{DateTime, NaiveDateTime, Utc};
use circular_queue::CircularQueue;

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

pub struct CostCount {}

impl RequestCoster for CostCount {
    fn cost(_r: &Request) -> u64 {
        1
    }
}

pub type IPReqCountValidator = CustomCostValidator<RequestIP, CountStateHolder, CostCount>;

pub struct CustomCostValidator<
    T: From<Request> + Hash + Eq + Into<BanTarget>,
    S: InitStateHolder,
    C: RequestCoster,
> {
    ban_desc: String,
    rules: Vec<BanRule>,
    target_data: HashMap<T, State<S>>,
    _phantom_c: PhantomData<C>,
}

impl<
        T: From<Request> + Hash + Eq + Clone + Into<BanTarget> + Debug,
        S: InitStateHolder,
        C: RequestCoster,
    > CustomCostValidator<T, S, C>
{
    pub fn new(rules: Vec<BanRuleConfig>, ban_desc: String) -> Self {
        let ip_data: HashMap<T, State<S>> = HashMap::new();
        CustomCostValidator {
            rules: rules.iter().map(|b| (*b).into()).collect(),
            ban_desc,
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

pub trait RequestCoster {
    fn cost(r: &Request) -> u64;
}

impl<
        T: From<Request> + Hash + Eq + Clone + Into<BanTarget> + Debug,
        S: InitStateHolder,
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
        if state.applied_rule_id.is_none() {
            state.base_costs.add(req.clone(), now);
            if !state
                .base_costs
                .is_above_limit(&(now - rule.reset_duration))
            {
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

        state.cost_since_last_ban += C::cost(&req);

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

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::marker::PhantomData;

    use anyhow::Error;
    use chrono::Duration;

    use crate::model::{BanRequest, BanTarget, Request};
    use crate::validator::generic_validator::{BanRule, IPReqCountValidator};
    use crate::validator::Validator;

    /// `get_default_validator` returns `IPCount` with
    /// next limits:
    ///
    /// 3 -> 1s ban, 2s reset
    ///
    /// 2 -> 3s ban, 6s reset
    ///
    /// 1 -> 4s ban, 8s reset
    fn get_default_validator() -> IPReqCountValidator {
        IPReqCountValidator {
            ban_desc: "".to_string(),
            rules: vec![
                BanRule {
                    limit: 3,
                    ban_duration: Duration::seconds(1),
                    reset_duration: Duration::seconds(2),
                },
                BanRule {
                    limit: 2,
                    ban_duration: Duration::seconds(3),
                    reset_duration: Duration::seconds(6),
                },
                BanRule {
                    limit: 1,
                    ban_duration: Duration::seconds(4),
                    reset_duration: Duration::seconds(8),
                },
            ],
            target_data: HashMap::new(),
            _phantom_c: PhantomData::default(),
        }
    }

    pub struct TestCase {
        //request, sleep before request
        pub input: Vec<(Request, Duration)>,
        pub want_last: Option<Result<Option<BanRequest>, Error>>,
        pub want_every: Option<Vec<Option<BanRequest>>>,
    }

    fn req_with_ip(ip: &str) -> Request {
        Request {
            remote_ip: ip.to_string(),
            host: "".to_string(),
            method: "".to_string(),
            path: "".to_string(),
            headers: Default::default(),
            body: "".to_string(),
        }
    }

    #[test]
    fn exceed_requests_leads_to_ban() {
        let tc = TestCase {
            input: vec![
                (req_with_ip("1.1.1.1"), Duration::seconds(0)),
                (req_with_ip("1.1.1.1"), Duration::seconds(0)),
                (req_with_ip("1.1.1.1"), Duration::seconds(0)),
            ],
            want_last: Some(Ok(Some(BanRequest {
                target: BanTarget {
                    ip: Some("1.1.1.1".to_string()),
                    user_agent: None,
                },
                reason: "".to_string(),
                ttl: 1,
                analyzer: "generic_counter".to_string(),
            }))),
            want_every: None,
        };

        run_test(tc);
    }

    #[test]
    fn not_exceed_requests_doesnt_lead_to_ban() {
        let tc = TestCase {
            input: vec![(req_with_ip("1.1.1.1"), Duration::seconds(0))],
            want_last: Some(Ok(None)),
            want_every: None,
        };

        run_test(tc);
    }

    #[test]
    fn waiting_before_last_request_doesnt_lead_to_ban() {
        let tc = TestCase {
            input: vec![
                (req_with_ip("1.1.1.1"), Duration::seconds(0)),
                (req_with_ip("1.1.1.1"), Duration::seconds(0)),
                (req_with_ip("1.1.1.1"), Duration::seconds(2)),
            ],
            want_last: Some(Ok(None)),
            want_every: Some(vec![None, None, None]),
        };

        run_test(tc);
    }

    #[test]
    fn rate_limit_doesnt_lead_to_ban() {
        let tc = TestCase {
            input: vec![
                (req_with_ip("1.1.1.1"), Duration::seconds(0)),
                (req_with_ip("1.1.1.1"), Duration::seconds(1)),
                (req_with_ip("1.1.1.1"), Duration::seconds(1)),
                (req_with_ip("1.1.1.1"), Duration::seconds(1)),
                (req_with_ip("1.1.1.1"), Duration::seconds(1)),
            ],
            want_last: None,
            want_every: Some(vec![None, None, None, None, None]),
        };

        run_test(tc);
    }

    #[test]
    fn request_while_banned_leads_to_nothing() {
        let tc = TestCase {
            input: vec![
                (req_with_ip("1.1.1.1"), Duration::seconds(0)),
                (req_with_ip("1.1.1.1"), Duration::seconds(0)),
                (req_with_ip("1.1.1.1"), Duration::seconds(0)),
                (req_with_ip("1.1.1.1"), Duration::seconds(0)),
            ],
            want_last: None,
            want_every: Some(vec![
                None,
                None,
                Some(BanRequest {
                    target: BanTarget {
                        ip: Some("1.1.1.1".to_string()),
                        user_agent: None,
                    },
                    reason: "".to_string(),
                    ttl: 1,
                    analyzer: "generic_counter".to_string(),
                }),
                None,
            ]),
        };

        run_test(tc);
    }

    #[test]
    fn exceed_requests_after_first_ban_leads_to_new_ban() {
        let tc = TestCase {
            input: vec![
                (req_with_ip("1.1.1.1"), Duration::seconds(0)),
                (req_with_ip("1.1.1.1"), Duration::seconds(0)),
                (req_with_ip("1.1.1.1"), Duration::seconds(0)),
                (req_with_ip("1.1.1.1"), Duration::seconds(0)),
                (req_with_ip("1.1.1.1"), Duration::seconds(0)),
            ],
            want_last: None,
            want_every: Some(vec![
                None,
                None,
                Some(BanRequest {
                    target: BanTarget {
                        ip: Some("1.1.1.1".to_string()),
                        user_agent: None,
                    },
                    reason: "".to_string(),
                    ttl: 1,
                    analyzer: "generic_counter".to_string(),
                }),
                None,
                Some(BanRequest {
                    target: BanTarget {
                        ip: Some("1.1.1.1".to_string()),
                        user_agent: None,
                    },
                    reason: "".to_string(),
                    ttl: 3,
                    analyzer: "generic_counter".to_string(),
                }),
            ]),
        };

        run_test(tc);
    }

    #[test]
    fn one_ip_provides_ban_only_for_itself() {
        let tc = TestCase {
            input: vec![
                (req_with_ip("1.1.1.1"), Duration::seconds(0)),
                (req_with_ip("2.2.2.2"), Duration::seconds(0)),
                (req_with_ip("3.3.3.3"), Duration::seconds(0)),
                (req_with_ip("3.3.3.3"), Duration::seconds(0)),
                (req_with_ip("3.3.3.3"), Duration::seconds(0)),
                (req_with_ip("1.1.1.1"), Duration::seconds(0)),
                (req_with_ip("1.1.1.1"), Duration::seconds(0)),
            ],
            want_last: None,
            want_every: Some(vec![
                None,
                None,
                None,
                None,
                Some(BanRequest {
                    target: BanTarget {
                        ip: Some("3.3.3.3".to_string()),
                        user_agent: None,
                    },
                    reason: "".to_string(),
                    ttl: 1,
                    analyzer: "generic_counter".to_string(),
                }),
                None,
                Some(BanRequest {
                    target: BanTarget {
                        ip: Some("1.1.1.1".to_string()),
                        user_agent: None,
                    },
                    reason: "".to_string(),
                    ttl: 1,
                    analyzer: "generic_counter".to_string(),
                }),
            ]),
        };

        run_test(tc);
    }

    #[test]
    fn after_reset_baned_by_first_rule() {
        let tc = TestCase {
            input: vec![
                (req_with_ip("1.1.1.1"), Duration::seconds(0)), // None
                (req_with_ip("1.1.1.1"), Duration::seconds(0)), // None
                (req_with_ip("1.1.1.1"), Duration::seconds(0)), // banned for 1s, 2s reset
                (req_with_ip("1.1.1.1"), Duration::seconds(2)), // currently unbanned
                (req_with_ip("1.1.1.1"), Duration::seconds(0)), // None
                (req_with_ip("1.1.1.1"), Duration::seconds(0)), // banned for 1s, 2s reset
            ],
            want_last: None,
            want_every: Some(vec![
                None,
                None,
                Some(BanRequest {
                    target: BanTarget {
                        ip: Some("1.1.1.1".to_string()),
                        user_agent: None,
                    },
                    reason: "".to_string(),
                    ttl: 1,
                    analyzer: "generic_counter".to_string(),
                }),
                None,
                None,
                Some(BanRequest {
                    target: BanTarget {
                        ip: Some("1.1.1.1".to_string()),
                        user_agent: None,
                    },
                    reason: "".to_string(),
                    ttl: 1,
                    analyzer: "generic_counter".to_string(),
                }),
            ]),
        };

        run_test(tc)
    }

    #[test]
    fn same_ban_after_exceed_last_limit_again() {
        let tc = TestCase {
            input: vec![
                (req_with_ip("1.1.1.1"), Duration::seconds(0)),
                (req_with_ip("1.1.1.1"), Duration::seconds(0)),
                (req_with_ip("1.1.1.1"), Duration::seconds(0)), // 1st
                (req_with_ip("1.1.1.1"), Duration::seconds(0)), //
                (req_with_ip("1.1.1.1"), Duration::seconds(0)), // 2nd
                (req_with_ip("1.1.1.1"), Duration::seconds(0)), // 3rd
                (req_with_ip("1.1.1.1"), Duration::seconds(0)), // 4th
            ],
            want_last: None,
            want_every: Some(vec![
                None,
                None,
                Some(BanRequest {
                    target: BanTarget {
                        ip: Some("1.1.1.1".to_string()),
                        user_agent: None,
                    },
                    reason: "".to_string(),
                    ttl: 1,
                    analyzer: "generic_counter".to_string(),
                }),
                None,
                Some(BanRequest {
                    target: BanTarget {
                        ip: Some("1.1.1.1".to_string()),
                        user_agent: None,
                    },
                    reason: "".to_string(),
                    ttl: 3,
                    analyzer: "generic_counter".to_string(),
                }),
                Some(BanRequest {
                    target: BanTarget {
                        ip: Some("1.1.1.1".to_string()),
                        user_agent: None,
                    },
                    reason: "".to_string(),
                    ttl: 4,
                    analyzer: "generic_counter".to_string(),
                }),
                Some(BanRequest {
                    target: BanTarget {
                        ip: Some("1.1.1.1".to_string()),
                        user_agent: None,
                    },
                    reason: "".to_string(),
                    ttl: 4,
                    analyzer: "generic_counter".to_string(),
                }),
            ]),
        };

        run_test(tc);
    }

    fn run_test(tc: TestCase) {
        let mut results = vec![];
        let mut v = get_default_validator();
        for (req, dur) in tc.input {
            std::thread::sleep(dur.to_std().unwrap());
            if let Ok(r) = v.validate(req) {
                results.push(r);
            }
        }

        assert!(tc.want_every.is_some() || tc.want_last.is_some());

        if let Some(ev) = tc.want_every {
            assert_eq!(ev, results)
        }
        if let Some(Ok(ev)) = tc.want_last {
            let res = results.pop().unwrap();
            assert_eq!(ev, res)
        }
    }
}
