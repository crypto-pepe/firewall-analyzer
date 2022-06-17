use std::cmp::min;
use std::collections::HashMap;

use anyhow::{Error, Result};
use chrono::prelude::*;
use regex::Regex;

use crate::model::{BanRequest, BanTarget, Request};
use crate::validation_provider::Validator;
use crate::validators::common::{BanRule, RulesError};
use crate::validators::requests_from_ip_cost::config::RequestPatternConfig;
use crate::validators::requests_from_ip_cost::Config;

use super::state::State;

pub struct RequestsFromIPCost {
    ban_description: String,
    rules: Vec<BanRule>,
    patterns: Vec<Pattern>,
    ip_ban_states: HashMap<String, State>,
    default_cost: u64,
}

pub struct Pattern {
    pub method: Option<String>,
    pub path_pattern: Regex,
    pub body_pattern: Option<Regex>,
    pub cost: u64,
}

impl TryFrom<RequestPatternConfig> for Pattern {
    type Error = Error;

    fn try_from(cfg: RequestPatternConfig) -> std::result::Result<Self, Self::Error> {
        let body_pattern = match cfg.body_regex {
            None => None,
            Some(r) => Some(Regex::new(r.as_str())?),
        };

        Ok(Self {
            method: cfg.method,
            path_pattern: Regex::new(cfg.path_regex.as_str())?,
            body_pattern,
            cost: cfg.cost,
        })
    }
}

impl RequestsFromIPCost {
    pub fn new(cfg: Config) -> Result<Self> {
        Ok(Self {
            rules: cfg
                .limits
                .iter()
                .map(|b| (*b).try_into())
                .collect::<Result<Vec<_>, _>>()?,
            patterns: cfg
                .patterns
                .into_iter()
                .map(|p| p.try_into())
                .collect::<Result<Vec<_>, _>>()?,
            ban_description: cfg.ban_description,
            default_cost: cfg.default_cost,
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

    fn evaluate_request(default_cost: u64, patterns: &[Pattern], req: &Request) -> u64 {
        // first found is used
        patterns
            .iter()
            .find_map(|p| {
                (p.method.clone().map(|a| a == req.method).unwrap_or(true)
                    && p.path_pattern.is_match(req.path.as_str())
                    && p.body_pattern
                        .clone()
                        .map(|p| p.is_match(req.body.as_str()))
                        .unwrap_or(true))
                .then(|| p.cost)
            })
            .unwrap_or(default_cost)
    }
}

impl Validator for RequestsFromIPCost {
    #[tracing::instrument(skip(self))]
    fn validate(&mut self, req: Request) -> Result<Option<BanRequest>, Error> {
        let ip = req.remote_ip.clone();
        let first_rule = self.rules.first().ok_or(RulesError::NotFound(0))?;
        let mut state = self
            .ip_ban_states
            .entry(ip.clone())
            .or_insert_with(|| State::new(first_rule.limit));

        let now = Utc::now();

        let cost =
            RequestsFromIPCost::evaluate_request(self.default_cost, self.patterns.as_ref(), &req);
        // Whether target is not banned

        if state.should_reset_timeout() {
            state.reset(cost, now);
            tracing::info!(action = "reset", ip = ip.as_str());
            return Ok(None);
        }

        match state.applied_rule_idx {
            None => {
                state.recent_requests.push((cost, now));
                state.clean_before(now - first_rule.reset_duration);

                if !state.is_limit_reached(now - first_rule.reset_duration) {
                    return Ok(None);
                }
                state.resets_at = now + first_rule.reset_duration;
                state.applied_rule_idx = Some(0);

                tracing::info!(
                    action = "ban",
                    ip = ip.as_str(),
                    limit = first_rule.limit,
                    ttl = first_rule.ban_duration.num_seconds()
                );
                Ok(Some(
                    self.ban(first_rule.ban_duration.num_seconds() as u32, ip),
                ))
            }
            Some(idx) => {
                state.cost_since_last_ban += cost;

                let rule_idx = min(idx + 1, self.rules.len() - 1);

                let applying_rule = self
                    .rules
                    .get(rule_idx)
                    .ok_or(RulesError::NotFound(rule_idx))?;
                if state.cost_since_last_ban >= applying_rule.limit {
                    state.resets_at = now + applying_rule.reset_duration;
                    state.cost_since_last_ban = 0;
                    state.applied_rule_idx = Some(rule_idx + 1);
                    tracing::info!(
                        action = "ban",
                        ip = ip.as_str(),
                        limit = applying_rule.limit,
                        ttl = applying_rule.ban_duration.num_seconds()
                    );
                    return Ok(Some(
                        self.ban(applying_rule.ban_duration.num_seconds() as u32, ip),
                    ));
                }
                Ok(None)
            }
        }
    }

    fn name(&self) -> String {
        "requests-from-ip-cost".into()
    }
}

#[cfg(test)]
mod tests {
    use anyhow::Error;
    use chrono::Duration;

    use crate::model::{BanRequest, BanTarget, Request};
    use crate::validation_provider::Validator;
    use crate::validators::common::BanRule;
    use crate::validators::requests_from_ip_cost::validator::Pattern;
    use crate::validators::requests_from_ip_cost::RequestsFromIPCost;

    /// `get_default_validator` returns `RequestsFromIPCost` with
    /// next limits:
    ///
    /// 30 -> 1s ban, 2s reset
    ///
    /// 20 -> 3s ban, 6s reset
    ///
    /// 10 -> 4s ban, 8s reset
    fn get_default_validator() -> RequestsFromIPCost {
        RequestsFromIPCost {
            ban_description: "".to_string(),
            rules: vec![
                BanRule {
                    limit: 30,
                    ban_duration: Duration::seconds(1),
                    reset_duration: Duration::seconds(2),
                },
                BanRule {
                    limit: 20,
                    ban_duration: Duration::seconds(3),
                    reset_duration: Duration::seconds(6),
                },
                BanRule {
                    limit: 10,
                    ban_duration: Duration::seconds(4),
                    reset_duration: Duration::seconds(8),
                },
            ],
            patterns: vec![
                Pattern {
                    method: Some("GET".to_string()),
                    path_pattern: regex::Regex::new(r"/cost/1.*").unwrap(),
                    body_pattern: Some(regex::Regex::new(r".*").unwrap()),
                    cost: 1,
                },
                Pattern {
                    method: Some("GET".to_string()),
                    path_pattern: regex::Regex::new(r"/cost/2.*").unwrap(),
                    body_pattern: Some(regex::Regex::new(r".*").unwrap()),
                    cost: 2,
                },
                Pattern {
                    method: Some("POST".to_string()),
                    path_pattern: regex::Regex::new(r"/cost/1.*").unwrap(),
                    body_pattern: Some(regex::Regex::new(r".*").unwrap()),
                    cost: 10,
                },
                Pattern {
                    method: Some("POST".to_string()),
                    path_pattern: regex::Regex::new(r"/cost/2.*").unwrap(),
                    body_pattern: Some(regex::Regex::new(r".*").unwrap()),
                    cost: 20,
                },
                Pattern {
                    method: Some("POST".to_string()),
                    path_pattern: regex::Regex::new(r"/123").unwrap(),
                    body_pattern: Some(regex::Regex::new(r"some payload").unwrap()),
                    cost: 15,
                },
                Pattern {
                    method: Some("POST".to_string()),
                    path_pattern: regex::Regex::new(r"/123").unwrap(),
                    body_pattern: Some(regex::Regex::new(r"big payload").unwrap()),
                    cost: 29,
                },
                Pattern {
                    method: Some("POST".to_string()),
                    path_pattern: regex::Regex::new(r".*").unwrap(),
                    body_pattern: Some(regex::Regex::new(r".*").unwrap()),
                    cost: 5,
                },
            ],
            ip_ban_states: Default::default(),
            default_cost: 0,
        }
    }

    fn base_req(ip: &str, method: &str, path: &str, body: &str) -> Request {
        Request {
            remote_ip: ip.to_string(),
            host: "".to_string(),
            method: method.to_string().to_uppercase(),
            path: path.to_string(),
            headers: Default::default(),
            body: body.to_string(),
        }
    }

    pub struct TestCase {
        //request, sleep before request
        pub input: Vec<(Request, Duration)>,
        pub want_last: Option<Result<Option<BanRequest>, Error>>,
        pub want_every: Option<Vec<Option<BanRequest>>>,
    }

    #[test]
    fn choose_correct_pattern() {
        let tc = TestCase {
            input: vec![
                (
                    base_req("1.1.1.1", "POST", "/unknown_pattern/1", ""),
                    Duration::seconds(0),
                ), // + 5
                (
                    base_req("1.1.1.1", "POST", "/unknown_pattern/2", ""),
                    Duration::seconds(0),
                ), // + 5
                (
                    base_req("1.1.1.1", "POST", "/unknown_pattern/3", ""),
                    Duration::seconds(0),
                ), // + 5
                (
                    base_req("1.1.1.1", "POST", "/aaaaaaa", ""),
                    Duration::seconds(0),
                ), // + 5
                (
                    base_req("1.1.1.1", "POST", "/bbbbb/2", ""),
                    Duration::seconds(0),
                ), // + 5
                (
                    base_req("1.1.1.1", "POST", "/unknown_pattern/2&123", ""),
                    Duration::seconds(0),
                ), // + 5 = 30
            ],
            want_last: None,
            want_every: Some(vec![
                None,
                None,
                None,
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

        run_test(tc);
    }

    #[test]
    #[ignore]
    fn choose_rule_with_max_cost() {
        let tc = TestCase {
            input: vec![
                (
                    base_req("1.1.1.1", "POST", "/123", "big payload"),
                    Duration::seconds(0),
                ), // + 29
                (
                    base_req("1.1.1.1", "GET", "/cost/1", ""),
                    Duration::seconds(0),
                ), // + 1 = 30
            ],
            want_last: None,
            want_every: Some(vec![
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
    fn exceed_cost_leads_to_ban() {
        let tc = TestCase {
            input: vec![
                (
                    base_req("1.1.1.1", "POST", "/cost/1", ""),
                    Duration::seconds(0),
                ),
                (
                    base_req("1.1.1.1", "POST", "/cost/1", ""),
                    Duration::seconds(0),
                ),
                (
                    base_req("1.1.1.1", "POST", "/cost/1", ""),
                    Duration::seconds(0),
                ),
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
    fn not_exceed_cost_doesnt_lead_to_ban() {
        let tc = TestCase {
            input: vec![
                (
                    base_req("1.1.1.1", "GET", "/cost/1", ""),
                    Duration::seconds(0),
                ),
                (
                    base_req("1.1.1.1", "GET", "/cost/1", ""),
                    Duration::seconds(0),
                ),
                (
                    base_req("1.1.1.1", "GET", "/cost/1", ""),
                    Duration::seconds(0),
                ),
                (
                    base_req("1.1.1.1", "GET", "/cost/1", ""),
                    Duration::seconds(0),
                ),
            ],
            want_last: Some(Ok(None)),
            want_every: None,
        };

        run_test(tc);
    }

    #[test]
    fn waiting_before_last_request_doesnt_lead_to_ban() {
        let tc = TestCase {
            input: vec![
                (
                    base_req("1.1.1.1", "POST", "/cost/1", ""),
                    Duration::seconds(0),
                ),
                (
                    base_req("1.1.1.1", "POST", "/cost/1", ""),
                    Duration::seconds(0),
                ),
                (
                    base_req("1.1.1.1", "POST", "/cost/2", ""),
                    Duration::seconds(2),
                ),
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
                (
                    base_req("1.1.1.1", "POST", "/cost/1", ""),
                    Duration::seconds(0),
                ),
                (
                    base_req("1.1.1.1", "POST", "/cost/1", ""),
                    Duration::seconds(1),
                ),
                (
                    base_req("1.1.1.1", "POST", "/cost/1", ""),
                    Duration::seconds(1),
                ),
                (
                    base_req("1.1.1.1", "POST", "/cost/1", ""),
                    Duration::seconds(1),
                ),
                (
                    base_req("1.1.1.1", "POST", "/cost/1", ""),
                    Duration::seconds(1),
                ),
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
                (
                    base_req("1.1.1.1", "POST", "/cost/1", ""),
                    Duration::seconds(0),
                ),
                (
                    base_req("1.1.1.1", "POST", "/cost/1", ""),
                    Duration::seconds(0),
                ),
                (
                    base_req("1.1.1.1", "POST", "/cost/1", ""),
                    Duration::seconds(0),
                ),
                (
                    base_req("1.1.1.1", "POST", "/cost/1", ""),
                    Duration::seconds(0),
                ),
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
                (
                    base_req("1.1.1.1", "POST", "/cost/1", ""),
                    Duration::seconds(0),
                ),
                (
                    base_req("1.1.1.1", "POST", "/cost/1", ""),
                    Duration::seconds(0),
                ),
                (
                    base_req("1.1.1.1", "POST", "/cost/1", ""),
                    Duration::seconds(0),
                ),
                (
                    base_req("1.1.1.1", "POST", "/cost/1", ""),
                    Duration::seconds(0),
                ),
                (
                    base_req("1.1.1.1", "POST", "/cost/1", ""),
                    Duration::seconds(0),
                ),
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
                (
                    base_req("1.1.1.1", "POST", "/cost/1", ""),
                    Duration::seconds(0),
                ),
                (
                    base_req("2.2.2.2", "POST", "/cost/1", ""),
                    Duration::seconds(0),
                ),
                (
                    base_req("3.3.3.3", "POST", "/cost/1", ""),
                    Duration::seconds(0),
                ),
                (
                    base_req("3.3.3.3", "POST", "/cost/1", ""),
                    Duration::seconds(0),
                ),
                (
                    base_req("3.3.3.3", "POST", "/cost/1", ""),
                    Duration::seconds(0),
                ),
                (
                    base_req("1.1.1.1", "POST", "/cost/1", ""),
                    Duration::seconds(0),
                ),
                (
                    base_req("1.1.1.1", "POST", "/cost/1", ""),
                    Duration::seconds(0),
                ),
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
                (
                    base_req("1.1.1.1", "POST", "/cost/1", ""),
                    Duration::seconds(0),
                ), // None
                (
                    base_req("1.1.1.1", "POST", "/cost/1", ""),
                    Duration::seconds(0),
                ), // None
                (
                    base_req("1.1.1.1", "POST", "/cost/1", ""),
                    Duration::seconds(0),
                ), // banned for 1s, 2s reset
                (
                    base_req("1.1.1.1", "POST", "/cost/1", ""),
                    Duration::seconds(2),
                ), // currently unbanned
                (
                    base_req("1.1.1.1", "POST", "/cost/1", ""),
                    Duration::seconds(0),
                ), // None
                (
                    base_req("1.1.1.1", "POST", "/cost/1", ""),
                    Duration::seconds(0),
                ), // banned for 1s, 2s reset
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
                (
                    base_req("1.1.1.1", "POST", "/cost/1", ""),
                    Duration::seconds(0),
                ),
                (
                    base_req("1.1.1.1", "POST", "/cost/1", ""),
                    Duration::seconds(0),
                ),
                (
                    base_req("1.1.1.1", "POST", "/cost/1", ""),
                    Duration::seconds(0),
                ), // 1st
                (
                    base_req("1.1.1.1", "POST", "/cost/1", ""),
                    Duration::seconds(0),
                ), //
                (
                    base_req("1.1.1.1", "POST", "/cost/1", ""),
                    Duration::seconds(0),
                ), // 2nd
                (
                    base_req("1.1.1.1", "POST", "/cost/1", ""),
                    Duration::seconds(0),
                ), // 3rd
                (
                    base_req("1.1.1.1", "POST", "/cost/1", ""),
                    Duration::seconds(0),
                ), // 4th
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
