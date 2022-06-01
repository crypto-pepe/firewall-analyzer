use std::cmp::min;
use std::collections::HashMap;

use anyhow::Error;
use chrono::prelude::*;

use super::state::State;

use crate::model::{BanRequest, BanTarget, Request};
use crate::validator::ip_count::{BanRule, BanRuleConfig};
use crate::validator::Validator;

pub struct IPCount {
    ban_desc: String,
    rules: Vec<BanRule>,
    ip_data: HashMap<String, State>,
}

impl IPCount {
    pub fn new(rules: Vec<BanRuleConfig>, ban_desc: String) -> Self {
        let ip_data = HashMap::new();
        IPCount {
            rules: rules.iter().map(|b| (*b).into()).collect(),
            ban_desc,
            ip_data,
        }
    }

    #[tracing::instrument(skip(self, rule_idx))]
    fn ban(&self, rule_idx: usize, ip: String) -> BanRequest {
        BanRequest {
            target: BanTarget {
                ip: Some(ip),
                user_agent: None,
            },
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

impl Validator for IPCount {
    #[tracing::instrument(skip(self))]
    fn validate(&mut self, req: Request) -> Result<Option<BanRequest>, Error> {
        let ip = req.remote_ip;
        let rule = self.rules.get(0).expect("at least one rule required");
        let mut data = self
            .ip_data
            .entry(ip.clone())
            .or_insert_with(|| State::new(rule.limit as usize));

        let now = Utc::now();

        // No ban now
        if data.applied_rule_idx.is_none() {
            data.recent_requests.push(now);
            if !data.recent_requests.is_full() {
                return Ok(None);
            }
            if *data.recent_requests.iter().last().unwrap() <= now - rule.reset_duration {
                return Ok(None);
            }

            data.resets_at = Utc::now() + rule.reset_duration;
            data.recent_requests.clear();
            data.requests_since_last_ban = 0;
            data.applied_rule_idx = Some(0);

            let rule = self.rules[0];
            tracing::info!(
                action = "ban",
                ip = ip.as_str(),
                limit = rule.limit,
                ttl = rule.ban_duration.num_seconds()
            );
            return Ok(Some(self.ban(0, ip)));
        }

        // was banned

        if data.should_reset_timeout() {
            data.reset(now);
            tracing::info!(action = "reset", ip = ip.as_str());
            return Ok(None);
        }

        data.requests_since_last_ban += 1;

        let rule_idx = data
            .applied_rule_idx
            .map_or(0, |v| min(v + 1, self.rules.len() - 1));

        if data.apply_rule_if_possible(&self.rules, rule_idx, now) {
            let rule = self.rules[rule_idx];
            tracing::info!(
                action = "ban",
                ip = ip.as_str(),
                limit = rule.limit,
                ttl = rule.ban_duration.num_seconds()
            );
            return Ok(Some(self.ban(rule_idx, ip)));
        }

        Ok(None)
    }

    fn name(&self) -> String {
        "ip_count".into()
    }
}

#[cfg(test)]
mod tests {
    use anyhow::Error;
    use chrono::Duration;

    use crate::model::{BanRequest, BanTarget, Request};
    use crate::validator::ip_count::{BanRule, IPCount};
    use crate::validator::Validator;

    /// `get_default_validator` returns `IPCount` with
    /// next limits:
    ///
    /// 3 -> 1s ban, 2s reset
    ///
    /// 2 -> 3s ban, 6s reset
    ///
    /// 1 -> 4s ban, 8s reset
    fn get_default_validator() -> IPCount {
        IPCount {
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
            ip_data: Default::default(),
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
                analyzer: "ip_count".to_string(),
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
                    analyzer: "ip_count".to_string(),
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
                    analyzer: "ip_count".to_string(),
                }),
                None,
                Some(BanRequest {
                    target: BanTarget {
                        ip: Some("1.1.1.1".to_string()),
                        user_agent: None,
                    },
                    reason: "".to_string(),
                    ttl: 3,
                    analyzer: "ip_count".to_string(),
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
                    analyzer: "ip_count".to_string(),
                }),
                None,
                Some(BanRequest {
                    target: BanTarget {
                        ip: Some("1.1.1.1".to_string()),
                        user_agent: None,
                    },
                    reason: "".to_string(),
                    ttl: 1,
                    analyzer: "ip_count".to_string(),
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
                    analyzer: "ip_count".to_string(),
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
                    analyzer: "ip_count".to_string(),
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
                    analyzer: "ip_count".to_string(),
                }),
                None,
                Some(BanRequest {
                    target: BanTarget {
                        ip: Some("1.1.1.1".to_string()),
                        user_agent: None,
                    },
                    reason: "".to_string(),
                    ttl: 3,
                    analyzer: "ip_count".to_string(),
                }),
                Some(BanRequest {
                    target: BanTarget {
                        ip: Some("1.1.1.1".to_string()),
                        user_agent: None,
                    },
                    reason: "".to_string(),
                    ttl: 4,
                    analyzer: "ip_count".to_string(),
                }),
                Some(BanRequest {
                    target: BanTarget {
                        ip: Some("1.1.1.1".to_string()),
                        user_agent: None,
                    },
                    reason: "".to_string(),
                    ttl: 4,
                    analyzer: "ip_count".to_string(),
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
