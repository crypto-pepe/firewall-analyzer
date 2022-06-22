use std::cmp::min;
use std::collections::HashMap;
use std::str::FromStr;

use anyhow::{Error, Result};
use chrono::prelude::*;
use regex::Regex;
use reqwest::header::USER_AGENT;

use crate::model::{BanRequest, BanTarget, Request};
use crate::validation_provider::Validator;
use crate::validators::common::{AppliedRule, BanRule, HeaderError, RulesError};
use crate::validators::requests_from_ua_counter::Config;

use super::state::State;

pub struct RequestsFromUACounter {
    ban_description: String,
    rules: Vec<BanRule>,
    patterns: Vec<Regex>,
    ua_ban_states: HashMap<String, State>,
}

impl RequestsFromUACounter {
    pub fn new(cfg: Config) -> Result<Self> {
        Ok(Self {
            rules: cfg
                .limits
                .iter()
                .map(|b| (*b).try_into())
                .collect::<Result<Vec<_>, _>>()?,
            ban_description: cfg.ban_description,
            patterns: cfg
                .patterns
                .iter()
                .map(|s| Regex::new(s))
                .collect::<Result<Vec<Regex>, _>>()?,
            ua_ban_states: HashMap::new(),
        })
    }

    #[tracing::instrument(skip(self))]
    fn ban(&self, ttl: u32, ua: String) -> BanRequest {
        BanRequest {
            target: BanTarget {
                ip: None,
                user_agent: Some(ua),
            },
            reason: self.ban_description.clone(),
            ttl,
        }
    }
}

impl Validator for RequestsFromUACounter {
    #[tracing::instrument(skip(self))]
    fn validate(&mut self, req: Request) -> Result<Option<BanRequest>, Error> {
        let ua = req
            .headers
            .iter()
            .find(|(k, _v)| k.eq_ignore_ascii_case(USER_AGENT.as_str()))
            .ok_or_else(|| HeaderError::NotFound(USER_AGENT.to_string()))?
            .1
            .clone();
        if !self
            .patterns
            .iter()
            .fold(false, |r, p| p.is_match(ua.as_str()) || r)
        {
            return Ok(None);
        }

        let rule = self.rules.get(0).ok_or(RulesError::NotFound(0))?;
        let mut state = self
            .ua_ban_states
            .entry(ua.clone())
            .or_insert_with(|| State::new(rule.limit as usize));

        let request_time: DateTime<Utc> = DateTime::from_str(&req.timestamp)?;

        if state.should_reset_timeout(request_time) {
            state.reset();
            state.push(request_time);
            tracing::info!(action = "reset", ua = ua.as_str());
            return Ok(None);
        }

        match &state.applied_rule {
            None => {
                state.push(request_time);
                if !state.is_above_limit(request_time - rule.reset_duration) {
                    return Ok(None);
                }

                state.apply_rule(AppliedRule {
                    rule_idx: 0,
                    resets_at: request_time + rule.reset_duration,
                });

                tracing::info!(
                    action = "ban",
                    ua = ua.as_str(),
                    limit = rule.limit,
                    ttl = rule.ban_duration.num_seconds()
                );
                Ok(Some(self.ban(rule.ban_duration.num_seconds() as u32, ua)))
            }
            Some(applied_rule) => {
                state.requests_since_last_ban += 1;

                let applying_rule_idx = min(applied_rule.rule_idx + 1, self.rules.len() - 1);

                let applying_rule = self
                    .rules
                    .get(applying_rule_idx)
                    .ok_or(RulesError::NotFound(applying_rule_idx))?;
                if state.requests_since_last_ban >= applying_rule.limit {
                    state.apply_rule(AppliedRule {
                        rule_idx: applying_rule_idx,
                        resets_at: request_time + applying_rule.reset_duration,
                    });
                    tracing::info!(
                        action = "ban",
                        ua = ua.as_str(),
                        limit = applying_rule.limit,
                        ttl = applying_rule.ban_duration.num_seconds()
                    );
                    return Ok(Some(
                        self.ban(applying_rule.ban_duration.num_seconds() as u32, ua),
                    ));
                }
                Ok(None)
            }
        }
    }

    fn name(&self) -> String {
        "requests-from-ua-counter".into()
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use anyhow::Error;
    use chrono::{Duration, Utc};
    use regex::Regex;
    use reqwest::header::USER_AGENT;

    use crate::model::Body::Original;
    use crate::model::{BanRequest, BanTarget, Request};
    use crate::validation_provider::Validator;
    use crate::validators::common::{BanRule, HeaderError};
    use crate::validators::RequestsFromUACounter;

    /// `get_default_validator` returns `RequestsFromUACounter` with
    /// next limits:
    ///
    /// 3 -> 1s ban, 2s reset
    ///
    /// 2 -> 3s ban, 6s reset
    ///
    /// 1 -> 4s ban, 8s reset
    fn get_default_validator() -> RequestsFromUACounter {
        RequestsFromUACounter {
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
            patterns: vec![
                Regex::new(r".*AAA.*").unwrap(),
                Regex::new(r".*BBB.*").unwrap(),
                Regex::new(r"123").unwrap(),
            ],
            ua_ban_states: Default::default(),
        }
    }

    pub struct TestCase {
        //request, sleep before request
        pub input: Vec<Request>,
        pub want_last: Option<Result<Option<BanRequest>, Error>>,
        pub want_every: Option<Vec<Option<BanRequest>>>,
        pub want_error: Option<Error>,
    }

    fn req_with_ua(ua: &str, wait_secs: i64) -> Request {
        Request {
            timestamp: (Utc::now() + Duration::seconds(wait_secs)).to_string(),
            remote_ip: "".to_string(),
            host: "".to_string(),
            method: "".to_string(),
            path: "".to_string(),
            headers: HashMap::from([("User-Agent".to_string(), ua.to_string())]),
            body: Original("".to_string()),
        }
    }

    fn empty_request(wait_secs: i64) -> Request {
        Request {
            timestamp: (Utc::now() + Duration::seconds(wait_secs)).to_string(),
            remote_ip: "".to_string(),
            host: "".to_string(),
            method: "".to_string(),
            path: "".to_string(),
            headers: HashMap::default(),
            body: Original("".to_string()),
        }
    }

    #[test]
    fn exceed_requests_leads_to_ban() {
        let tc = TestCase {
            input: vec![
                req_with_ua("AAA", 0),
                req_with_ua("AAA", 0),
                req_with_ua("AAA", 0),
            ],
            want_last: Some(Ok(Some(BanRequest {
                target: BanTarget {
                    ip: None,
                    user_agent: Some("AAA".to_string()),
                },
                reason: "".to_string(),
                ttl: 1,
            }))),
            want_every: None,
            want_error: None,
        };

        run_test(tc);
    }

    #[test]
    fn not_exceed_requests_doesnt_lead_to_ban() {
        let tc = TestCase {
            input: vec![req_with_ua("123", 0)],
            want_last: Some(Ok(None)),
            want_every: None,
            want_error: None,
        };

        run_test(tc);
    }

    #[test]
    fn waiting_before_last_request_doesnt_lead_to_ban() {
        let tc = TestCase {
            input: vec![
                req_with_ua("123", 0),
                req_with_ua("123", 0),
                req_with_ua("123", 2),
            ],
            want_last: Some(Ok(None)),
            want_every: Some(vec![None, None, None]),
            want_error: None,
        };

        run_test(tc);
    }

    #[test]
    fn rate_limit_doesnt_lead_to_ban() {
        let tc = TestCase {
            input: vec![
                req_with_ua("123", 0),
                req_with_ua("123", 1),
                req_with_ua("123", 2),
                req_with_ua("123", 3),
                req_with_ua("123", 4),
            ],
            want_last: None,
            want_every: Some(vec![None, None, None, None, None]),
            want_error: None,
        };

        run_test(tc);
    }

    #[test]
    fn request_while_banned_leads_to_nothing() {
        let tc = TestCase {
            want_error: None,
            input: vec![
                req_with_ua("123", 0),
                req_with_ua("123", 0),
                req_with_ua("123", 0),
                req_with_ua("123", 0),
            ],
            want_last: None,
            want_every: Some(vec![
                None,
                None,
                Some(BanRequest {
                    target: BanTarget {
                        ip: None,
                        user_agent: Some("123".to_string()),
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
            want_error: None,
            input: vec![
                req_with_ua("123", 0),
                req_with_ua("123", 0),
                req_with_ua("123", 0),
                req_with_ua("123", 0),
                req_with_ua("123", 0),
            ],
            want_last: None,
            want_every: Some(vec![
                None,
                None,
                Some(BanRequest {
                    target: BanTarget {
                        ip: None,
                        user_agent: Some("123".to_string()),
                    },
                    reason: "".to_string(),
                    ttl: 1,
                }),
                None,
                Some(BanRequest {
                    target: BanTarget {
                        ip: None,
                        user_agent: Some("123".to_string()),
                    },
                    reason: "".to_string(),
                    ttl: 3,
                }),
            ]),
        };

        run_test(tc);
    }

    #[test]
    fn one_ua_provides_ban_only_for_itself() {
        let tc = TestCase {
            want_error: None,
            input: vec![
                req_with_ua("123", 0),
                req_with_ua("AAA", 0),
                req_with_ua("BBB", 0),
                req_with_ua("BBB", 0),
                req_with_ua("BBB", 0),
                req_with_ua("BBB", 0),
                req_with_ua("BBB", 0),
                req_with_ua("123", 0),
                req_with_ua("123", 0),
            ],
            want_last: None,
            want_every: Some(vec![
                None,
                None,
                None,
                None,
                Some(BanRequest {
                    target: BanTarget {
                        ip: None,
                        user_agent: Some("BBB".to_string()),
                    },
                    reason: "".to_string(),
                    ttl: 1,
                }),
                None,
                Some(BanRequest {
                    target: BanTarget {
                        ip: None,
                        user_agent: Some("BBB".to_string()),
                    },
                    reason: "".to_string(),
                    ttl: 3,
                }),
                None,
                Some(BanRequest {
                    target: BanTarget {
                        ip: None,
                        user_agent: Some("123".to_string()),
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
            want_error: None,
            input: vec![
                req_with_ua("123", 0), // None
                req_with_ua("123", 0), // None
                req_with_ua("123", 0), // banned for 1s, 2s reset
                req_with_ua("123", 3), // currently unbanned
                req_with_ua("123", 3), // None
                req_with_ua("123", 3), // banned for 1s, 2s reset
            ],
            want_last: None,
            want_every: Some(vec![
                None,
                None,
                Some(BanRequest {
                    target: BanTarget {
                        ip: None,
                        user_agent: Some("123".to_string()),
                    },
                    reason: "".to_string(),
                    ttl: 1,
                }),
                None,
                None,
                Some(BanRequest {
                    target: BanTarget {
                        ip: None,
                        user_agent: Some("123".to_string()),
                    },
                    reason: "".to_string(),
                    ttl: 1,
                }),
            ]),
        };

        run_test(tc)
    }

    #[test]
    fn error_when_no_user_agent() {
        let tc = TestCase {
            want_error: Some(HeaderError::NotFound(USER_AGENT.to_string()).into()),
            input: vec![empty_request(0)],
            want_last: None,
            want_every: None,
        };

        run_test(tc)
    }

    #[test]
    fn do_nothing_if_user_agent_doesnt_match_pattern() {
        let tc = TestCase {
            want_error: None,
            input: vec![
                req_with_ua("some ua", 0),
                req_with_ua("some ua", 0),
                req_with_ua("some ua", 0),
                req_with_ua("some ua", 0),
            ],
            want_last: None,
            want_every: Some(vec![None, None, None, None]),
        };

        run_test(tc)
    }

    #[test]
    fn same_ban_after_exceed_last_limit_again() {
        let tc = TestCase {
            want_error: None,
            input: vec![
                req_with_ua("123", 0),
                req_with_ua("123", 0),
                req_with_ua("123", 0), // 1st
                req_with_ua("123", 0), //
                req_with_ua("123", 0), // 2nd
                req_with_ua("123", 0), // 3rd
                req_with_ua("123", 0), // 4th
            ],
            want_last: None,
            want_every: Some(vec![
                None,
                None,
                Some(BanRequest {
                    target: BanTarget {
                        ip: None,
                        user_agent: Some("123".to_string()),
                    },
                    reason: "".to_string(),
                    ttl: 1,
                }),
                None,
                Some(BanRequest {
                    target: BanTarget {
                        ip: None,
                        user_agent: Some("123".to_string()),
                    },
                    reason: "".to_string(),
                    ttl: 3,
                }),
                Some(BanRequest {
                    target: BanTarget {
                        ip: None,
                        user_agent: Some("123".to_string()),
                    },
                    reason: "".to_string(),
                    ttl: 4,
                }),
                Some(BanRequest {
                    target: BanTarget {
                        ip: None,
                        user_agent: Some("123".to_string()),
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
            match v.validate(req) {
                Ok(r) => results.push(r),
                Err(got) => match tc.want_error {
                    None => panic!("error {:?} not expected", got),
                    Some(ref expect) => assert_eq!(got.to_string(), expect.to_string()),
                },
            }
        }

        assert!(tc.want_every.is_some() || tc.want_last.is_some() || tc.want_error.is_some());

        if let Some(ev) = tc.want_every {
            assert_eq!(ev, results)
        }
        if let Some(Ok(ev)) = tc.want_last {
            let res = results.pop().unwrap();
            assert_eq!(ev, res)
        }
    }
}
