use crate::model::{BanTarget, Request};
use crate::validator::generic_validator::count::{CostCount, CountStateHolder};
use crate::validator::generic_validator::rule::BanRule;
use crate::validator::generic_validator::{BaseCostCounter, CustomCostValidator, RequestCoster};
use chrono::{DateTime, Utc};
use serde::Deserialize;
use std::cmp::max;
use std::fmt::{Debug, Formatter};

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

#[derive(Deserialize)]
pub struct CostRuleConfig {
    pub method: String,
    pub path_pattern: String,
    pub body_pattern: String,
    pub cost: u64,
}

pub struct CostRule {
    pub method: String,
    pub path_pattern: regex::Regex,
    pub body_pattern: regex::Regex,
    pub cost: u64,
}

impl TryFrom<CostRuleConfig> for CostRule {
    type Error = regex::Error;

    fn try_from(cfg: CostRuleConfig) -> Result<Self, Self::Error> {
        let p = regex::Regex::new(&*cfg.path_pattern)?;
        let b = regex::Regex::new(&*cfg.body_pattern)?;
        Ok(CostRule {
            method: cfg.method,
            path_pattern: p,
            body_pattern: b,
            cost: cfg.cost,
        })
    }
}

pub struct RequestCostByRegex {
    pub rules: Vec<CostRule>,
}

impl RequestCoster for RequestCostByRegex {
    fn cost(&self, r: &Request) -> u64 {
        self.rules.iter().fold(0, |cost, c| {
            if r.method == c.method
                && c.path_pattern.is_match(&*r.path)
                && c.body_pattern.is_match(&*r.body)
            {
                max(cost, c.cost)
            } else {
                cost
            }
        })
    }
}

#[derive(Debug)]
pub struct CostCounter {
    pub limit: u64,
    pub reqs: Vec<(u64, DateTime<Utc>)>,
}

impl BaseCostCounter for CostCounter {
    fn new(r: &BanRule) -> Self {
        CostCounter {limit:r.limit, reqs: vec![] }
    }

    fn add(&mut self, cost: u64, time: DateTime<Utc>) {
        self.reqs.push((cost, time))
    }

    fn latest_value_added_at(&self) -> Option<DateTime<Utc>> {
        self.reqs.last().copied().map(|(cost, time)| time)
    }

    fn is_above_limit(&self, time: &DateTime<Utc>) -> bool {
        self.reqs.iter().filter(|(cost, t)| t> time).fold(0u64, |c, (cost, _)| c+cost) > self.limit
    }

    fn clear(&mut self) {
        self.reqs.clear();
    }
}

pub type IPRequestCostValidator =
    CustomCostValidator<RequestIP, CostCounter, RequestCostByRegex>;

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use anyhow::Error;
    use chrono::Duration;

    use crate::model::{BanRequest, BanTarget, Request};
    use crate::validator::generic_validator::BaseCostCounter;
    use crate::validator::generic_validator::count::CostCount;
    use crate::validator::generic_validator::rule::BanRule;
    use crate::validator::ip_count::IPReqCountValidator;
    use crate::validator::request_cost::{CostCounter, CostRule, IPRequestCostValidator, RequestCostByRegex};
    use crate::validator::Validator;

    /// `get_default_validator` returns `IPRequestCostValidator` with
    /// next limits:
    ///
    /// 60 -> 1s ban, 2s reset
    ///
    /// 30 -> 3s ban, 6s reset
    ///
    /// 10 -> 4s ban, 8s reset
    fn get_default_validator() -> IPRequestCostValidator {
        IPRequestCostValidator {
            ban_desc: "".to_string(),
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
            target_data: HashMap::new(),
            name: "generic_counter".to_string(),
            coster: RequestCostByRegex{ rules: vec![
                CostRule{
                    method: "GET".to_string(),
                    path_pattern: regex::Regex::new("/cost/1.*").unwrap(),
                    body_pattern: regex::Regex::new(".*").unwrap(),
                    cost: 1
                },
                CostRule{
                    method: "GET".to_string(),
                    path_pattern: regex::Regex::new("/cost/2.*").unwrap(),
                    body_pattern: regex::Regex::new(".*").unwrap(),
                    cost: 2
                },
                CostRule{
                    method: "POST".to_string(),
                    path_pattern: regex::Regex::new("/cost/2.*").unwrap(),
                    body_pattern: regex::Regex::new(".*").unwrap(),
                    cost: 10
                },
                CostRule{
                    method: "POST".to_string(),
                    path_pattern: regex::Regex::new("/cost/2.*").unwrap(),
                    body_pattern: regex::Regex::new(".*").unwrap(),
                    cost: 20
                }
            ] },
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
    fn exceed_requests_leads_to_ban() {
        let tc = TestCase {
            input: vec![
                (base_req("1.1.1.1","POST", "/cost/1", ""), Duration::seconds(0)),
                (base_req("1.1.1.1","POST", "/cost/1", ""), Duration::seconds(0)),
                (base_req("1.1.1.1","POST", "/cost/1", ""), Duration::seconds(0)),
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
            input: vec![(base_req("1.1.1.1","GET", "/cost/1",""), Duration::seconds(0))],
            want_last: Some(Ok(None)),
            want_every: None,
        };

        run_test(tc);
    }

    #[test]
    fn waiting_before_last_request_doesnt_lead_to_ban() {
        let tc = TestCase {
            input: vec![
                // (req_with_ip("1.1.1.1"), Duration::seconds(0)),
                // (req_with_ip("1.1.1.1"), Duration::seconds(0)),
                // (req_with_ip("1.1.1.1"), Duration::seconds(2)),
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
                // (req_with_ip("1.1.1.1"), Duration::seconds(0)),
                // (req_with_ip("1.1.1.1"), Duration::seconds(1)),
                // (req_with_ip("1.1.1.1"), Duration::seconds(1)),
                // (req_with_ip("1.1.1.1"), Duration::seconds(1)),
                // (req_with_ip("1.1.1.1"), Duration::seconds(1)),
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
                // (req_with_ip("1.1.1.1"), Duration::seconds(0)),
                // (req_with_ip("1.1.1.1"), Duration::seconds(0)),
                // (req_with_ip("1.1.1.1"), Duration::seconds(0)),
                // (req_with_ip("1.1.1.1"), Duration::seconds(0)),
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
                // (req_with_ip("1.1.1.1"), Duration::seconds(0)),
                // (req_with_ip("1.1.1.1"), Duration::seconds(0)),
                // (req_with_ip("1.1.1.1"), Duration::seconds(0)),
                // (req_with_ip("1.1.1.1"), Duration::seconds(0)),
                // (req_with_ip("1.1.1.1"), Duration::seconds(0)),
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
                // (req_with_ip("1.1.1.1"), Duration::seconds(0)),
                // (req_with_ip("2.2.2.2"), Duration::seconds(0)),
                // (req_with_ip("3.3.3.3"), Duration::seconds(0)),
                // (req_with_ip("3.3.3.3"), Duration::seconds(0)),
                // (req_with_ip("3.3.3.3"), Duration::seconds(0)),
                // (req_with_ip("1.1.1.1"), Duration::seconds(0)),
                // (req_with_ip("1.1.1.1"), Duration::seconds(0)),
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
                // (req_with_ip("1.1.1.1"), Duration::seconds(0)), // None
                // (req_with_ip("1.1.1.1"), Duration::seconds(0)), // None
                // (req_with_ip("1.1.1.1"), Duration::seconds(0)), // banned for 1s, 2s reset
                // (req_with_ip("1.1.1.1"), Duration::seconds(2)), // currently unbanned
                // (req_with_ip("1.1.1.1"), Duration::seconds(0)), // None
                // (req_with_ip("1.1.1.1"), Duration::seconds(0)), // banned for 1s, 2s reset
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
                // (req_with_ip("1.1.1.1"), Duration::seconds(0)),
                // (req_with_ip("1.1.1.1"), Duration::seconds(0)),
                // (req_with_ip("1.1.1.1"), Duration::seconds(0)), // 1st
                // (req_with_ip("1.1.1.1"), Duration::seconds(0)), //
                // (req_with_ip("1.1.1.1"), Duration::seconds(0)), // 2nd
                // (req_with_ip("1.1.1.1"), Duration::seconds(0)), // 3rd
                // (req_with_ip("1.1.1.1"), Duration::seconds(0)), // 4th
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
