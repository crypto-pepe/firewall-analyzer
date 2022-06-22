use std::cmp::min;
use std::collections::HashMap;
use std::str::FromStr;

use anyhow::{Error, Result};
use chrono::prelude::*;

use super::state::State;

use crate::model::{BanRequest, BanTarget, Request};
use crate::validation_provider::Validator;
use crate::validators::common::{BanRule, RulesError};
use crate::validators::requests_from_ip_counter::Config;

pub struct RequestsFromIPCounter {
    ban_description: String,
    rules: Vec<BanRule>,
    ip_ban_states: HashMap<String, State>,
}

impl RequestsFromIPCounter {
    pub fn new(cfg: Config) -> Result<Self> {
        Ok(Self {
            rules: cfg
                .limits
                .iter()
                .map(|b| (*b).try_into())
                .collect::<Result<Vec<_>, _>>()?,
            ban_description: cfg.ban_description,
            ip_ban_states: HashMap::new(),
        })
    }

    #[tracing::instrument(skip(self))]
    fn ban(&self, ttl: u32, ip: String) -> BanRequest {
        BanRequest {
            target: BanTarget {
                ip: Some(ip),
                user_agent: None,
            },
            reason: self.ban_description.clone(),
            ttl,
        }
    }
}

impl Validator for RequestsFromIPCounter {
    #[tracing::instrument(skip(self))]
    fn validate(&mut self, req: Request) -> Result<Option<BanRequest>, Error> {
        let ip = req.remote_ip;
        let rule = self.rules.get(0).ok_or(RulesError::NotFound(0))?;
        let mut state = self
            .ip_ban_states
            .entry(ip.clone())
            .or_insert_with(|| State::new(rule.limit as usize));

        let request_time: DateTime<Utc> = DateTime::from_str(&req.timestamp)?;

        // Whether target is not banned
        if state.applied_rule_idx.is_none() {
            state.recent_requests.push(request_time);
            if !state.recent_requests.is_full() {
                return Ok(None);
            }
            if *state.recent_requests.iter().last().unwrap() <= request_time - rule.reset_duration {
                return Ok(None);
            }

            state.resets_at = request_time + rule.reset_duration;
            state.recent_requests.clear();
            state.requests_since_last_ban = 0;
            state.applied_rule_idx = Some(0);

            let rule = self.rules[0];
            tracing::info!(
                action = "ban",
                ip = ip.as_str(),
                limit = rule.limit,
                ttl = rule.ban_duration.num_seconds()
            );
            return Ok(Some(self.ban(rule.ban_duration.num_seconds() as u32, ip)));
        }

        if state.should_reset_timeout(request_time) {
            state.reset();
            state.recent_requests.push(request_time);
            tracing::info!(action = "reset", ip = ip.as_str());
            return Ok(None);
        }

        state.requests_since_last_ban += 1;

        let rule_idx = state
            .applied_rule_idx
            .map_or(0, |v| min(v + 1, self.rules.len() - 1));

        if apply_rule_if_possible(state, &self.rules, rule_idx, request_time)? {
            let rule = self.rules[rule_idx];
            tracing::info!(
                action = "ban",
                ip = ip.as_str(),
                limit = rule.limit,
                ttl = rule.ban_duration.num_seconds()
            );
            return Ok(Some(self.ban(rule.ban_duration.num_seconds() as u32, ip)));
        }

        Ok(None)
    }

    fn name(&self) -> String {
        "requests-from-ip-counter".into()
    }
}

fn apply_rule_if_possible(
    state: &mut State,
    rules: &[BanRule],
    rule_idx: usize,
    last_request_time: DateTime<Utc>,
) -> Result<bool, RulesError> {
    let rule = rules.get(rule_idx).ok_or(RulesError::NotFound(rule_idx))?;
    if state.requests_since_last_ban >= rule.limit {
        state.resets_at = last_request_time + rule.reset_duration;
        state.requests_since_last_ban = 0;
        state.applied_rule_idx = Some(rule_idx + 1);
        return Ok(true);
    }
    Ok(false)
}

#[cfg(test)]
mod tests {
    use anyhow::Error;
    use chrono::{Duration, Utc};

    use crate::model::{BanRequest, BanTarget, Body, Request};
    use crate::validation_provider::Validator;
    use crate::validators::common::BanRule;
    use crate::validators::RequestsFromIPCounter;

    /// `get_default_validator` returns `IPCount` with
    /// next limits:
    ///
    /// 3 -> 1s ban, 2s reset
    ///
    /// 2 -> 3s ban, 6s reset
    ///
    /// 1 -> 4s ban, 8s reset
    fn get_default_validator() -> RequestsFromIPCounter {
        RequestsFromIPCounter {
            ban_description: "".to_string(),
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
            ip_ban_states: Default::default(),
        }
    }

    pub struct TestCase {
        //request, sleep before request
        pub input: Vec<Request>,
        pub want_last: Option<Result<Option<BanRequest>, Error>>,
        pub want_every: Option<Vec<Option<BanRequest>>>,
    }

    fn req_with_ip(ip: &str, wait_secs: i64) -> Request {
        Request {
            timestamp: (Utc::now() + Duration::seconds(wait_secs)).to_string(),
            remote_ip: ip.to_string(),
            host: "".to_string(),
            method: "".to_string(),
            path: "".to_string(),
            headers: Default::default(),
            body: Body::Original("".to_string()),
        }
    }

    #[test]
    fn exceed_requests_leads_to_ban() {
        let tc = TestCase {
            input: vec![
                req_with_ip("1.1.1.1", 0),
                req_with_ip("1.1.1.1", 0),
                req_with_ip("1.1.1.1", 0),
            ],
            want_last: Some(Ok(Some(BanRequest {
                target: BanTarget {
                    ip: Some("1.1.1.1".to_string()),
                    user_agent: None,
                },
                reason: "".to_string(),
                ttl: 1,
            }))),
            want_every: None,
        };

        run_test(tc);
    }

    #[test]
    fn not_exceed_requests_doesnt_lead_to_ban() {
        let tc = TestCase {
            input: vec![req_with_ip("1.1.1.1", 0)],
            want_last: Some(Ok(None)),
            want_every: None,
        };

        run_test(tc);
    }

    #[test]
    fn waiting_before_last_request_doesnt_lead_to_ban() {
        let tc = TestCase {
            input: vec![
                req_with_ip("1.1.1.1", 0),
                req_with_ip("1.1.1.1", 0),
                req_with_ip("1.1.1.1", 2),
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
                req_with_ip("1.1.1.1", 0),
                req_with_ip("1.1.1.1", 1),
                req_with_ip("1.1.1.1", 2),
                req_with_ip("1.1.1.1", 3),
                req_with_ip("1.1.1.1", 4),
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
                req_with_ip("1.1.1.1", 0),
                req_with_ip("1.1.1.1", 0),
                req_with_ip("1.1.1.1", 0),
                req_with_ip("1.1.1.1", 0),
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
                req_with_ip("1.1.1.1", 0),
                req_with_ip("1.1.1.1", 0),
                req_with_ip("1.1.1.1", 0),
                req_with_ip("1.1.1.1", 0),
                req_with_ip("1.1.1.1", 0),
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
                }),
                None,
                Some(BanRequest {
                    target: BanTarget {
                        ip: Some("1.1.1.1".to_string()),
                        user_agent: None,
                    },
                    reason: "".to_string(),
                    ttl: 3,
                }),
            ]),
        };

        run_test(tc);
    }

    #[test]
    fn one_ip_provides_ban_only_for_itself() {
        let tc = TestCase {
            input: vec![
                req_with_ip("1.1.1.1", 0),
                req_with_ip("2.2.2.2", 0),
                req_with_ip("3.3.3.3", 0),
                req_with_ip("3.3.3.3", 0),
                req_with_ip("3.3.3.3", 0),
                req_with_ip("1.1.1.1", 0),
                req_with_ip("1.1.1.1", 0),
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
                }),
                None,
                Some(BanRequest {
                    target: BanTarget {
                        ip: Some("1.1.1.1".to_string()),
                        user_agent: None,
                    },
                    reason: "".to_string(),
                    ttl: 1,
                }),
            ]),
        };

        run_test(tc);
    }

    #[test]
    fn after_reset_baned_by_first_rule() {
        let tc = TestCase {
            input: vec![
                req_with_ip("1.1.1.1", 0), // None
                req_with_ip("1.1.1.1", 0), // None
                req_with_ip("1.1.1.1", 0), // banned for 1s, 2s reset
                req_with_ip("1.1.1.1", 3), // currently unbanned
                req_with_ip("1.1.1.1", 3), // None
                req_with_ip("1.1.1.1", 3), // banned for 1s, 2s reset
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
                }),
            ]),
        };

        run_test(tc)
    }

    #[test]
    fn same_ban_after_exceed_last_limit_again() {
        let tc = TestCase {
            input: vec![
                req_with_ip("1.1.1.1", 0),
                req_with_ip("1.1.1.1", 0),
                req_with_ip("1.1.1.1", 0), // 1st
                req_with_ip("1.1.1.1", 0), //
                req_with_ip("1.1.1.1", 0), // 2nd
                req_with_ip("1.1.1.1", 0), // 3rd
                req_with_ip("1.1.1.1", 0), // 4th
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
                }),
                None,
                Some(BanRequest {
                    target: BanTarget {
                        ip: Some("1.1.1.1".to_string()),
                        user_agent: None,
                    },
                    reason: "".to_string(),
                    ttl: 3,
                }),
                Some(BanRequest {
                    target: BanTarget {
                        ip: Some("1.1.1.1".to_string()),
                        user_agent: None,
                    },
                    reason: "".to_string(),
                    ttl: 4,
                }),
                Some(BanRequest {
                    target: BanTarget {
                        ip: Some("1.1.1.1".to_string()),
                        user_agent: None,
                    },
                    reason: "".to_string(),
                    ttl: 4,
                }),
            ]),
        };

        run_test(tc);
    }

    fn run_test(tc: TestCase) {
        let mut results = vec![];
        let mut v = get_default_validator();
        for req in tc.input {
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
