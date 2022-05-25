use crate::model::{BanRequest, BanTarget, Request};
use crate::validator::Validator;
use anyhow::Error;
use chrono::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct BanRuleConfig {
    pub limit: u64,
    pub ban_duration: duration_string::DurationString,
    pub reset_duration: duration_string::DurationString,
}

pub struct BanRule {
    pub limit: u64,
    pub ban_duration: chrono::Duration,
    pub reset_duration: chrono::Duration,
}

impl From<BanRuleConfig> for BanRule {
    fn from(brc: BanRuleConfig) -> Self {
        BanRule {
            limit: brc.limit,
            ban_duration: chrono::Duration::from_std(brc.ban_duration.into()).unwrap(),
            reset_duration: chrono::Duration::from_std(brc.reset_duration.into()).unwrap(),
        }
    }
}

#[derive(Debug)]
pub struct Data {
    count: u64,
    last: DateTime<Utc>,
    banned_till: DateTime<Utc>,
}

impl Default for Data {
    fn default() -> Self {
        Data {
            count: 0,
            last: Utc::now(),
            banned_till: DateTime::<Utc>::from_utc(NaiveDateTime::from_timestamp(0, 0), Utc),
        }
    }
}

pub struct IPCount {
    pub ban_desc: String,
    pub rules: Vec<BanRule>,
    pub ip_data: HashMap<String, Data>,
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
}

impl Validator for IPCount {
    // todo refactor
    fn validate(&mut self, req: Request) -> Result<Option<BanRequest>, Error> {
        let ip = req.remote_ip;
        let mut data = self.ip_data.entry(ip.clone()).or_insert(Data::default());
        data.count += 1;

        let rule_idx = self.rules.binary_search_by(|r| r.limit.cmp(&data.count));

        // first possibly applicable rule
        let rule = match rule_idx {
            Ok(idx) => self.rules.get(idx),
            Err(_) => return Ok(None),
        }
        .expect("no rules set");

        // reset duration exceeds from last request
        if data.last + rule.ban_duration <= Utc::now() {
            data.count = 1
        }
        data.last = Utc::now();

        if rule.limit <= data.count {
            data.banned_till = data.last + rule.ban_duration;
            Ok(Some(BanRequest {
                target: BanTarget {
                    ip: Some(ip.clone()),
                    user_agent: None,
                },
                reason: self.ban_desc.clone(),
                ttl: rule.ban_duration.num_seconds() as u32,
                analyzer: self.name(),
            }))
        } else {
            Ok(None)
        }
    }

    fn name(&self) -> String {
        "ip_count".into()
    }
}

#[cfg(test)]
mod tests {
    use crate::model::{BanRequest, BanTarget, Request};
    use crate::validator::ip_count::{BanRule, IPCount};
    use crate::validator::Validator;
    use anyhow::Error;
    use chrono::Duration;

    fn print_ip_data(v: &IPCount) {
        for (ip, data) in &v.ip_data {
            println!("{}: {:?}", ip, data)
        }
    }

    /// get_default_validator returns IPCount with
    /// next limits:
    ///
    /// 2 -> 1s ban, 2s reset
    ///
    /// 5 -> 3s ban, 6s reset
    ///
    /// 10 -> 4s ban, 8s reset
    fn get_default_validator() -> IPCount {
        IPCount {
            ban_desc: "".to_string(),
            rules: vec![
                BanRule {
                    limit: 2,
                    ban_duration: Duration::seconds(1),
                    reset_duration: Duration::seconds(2),
                },
                BanRule {
                    limit: 5,
                    ban_duration: Duration::seconds(3),
                    reset_duration: Duration::seconds(6),
                },
                BanRule {
                    limit: 10,
                    ban_duration: Duration::seconds(4),
                    reset_duration: Duration::seconds(8),
                },
            ],
            ip_data: Default::default(),
        }
    }

    pub struct TestCase {
        pub input: Vec<(Request, Duration)>, //request, sleep before request
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

    #[test]
    fn not_exceed_requests_doesnt_lead_to_ban() {
        let tc = TestCase {
            input: vec![(req_with_ip("1.1.1.1"), Duration::seconds(0))],
            want_last: Some(Ok(None)),
            want_every: None,
        };

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

    #[test]
    fn waiting_before_last_request_doesnt_lead_to_ban() {
        let tc = TestCase {
            input: vec![
                (req_with_ip("1.1.1.1"), Duration::seconds(0)),
                (req_with_ip("1.1.1.1"), Duration::seconds(2)),
            ],
            want_last: Some(Ok(None)),
            want_every: Some(vec![None, None]),
        };

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

    #[test]
    fn request_while_banned_leads_to_nothing() {
        let tc = TestCase {
            input: vec![
                (req_with_ip("1.1.1.1"), Duration::seconds(0)),
                (req_with_ip("1.1.1.1"), Duration::seconds(0)),
                (req_with_ip("1.1.1.1"), Duration::seconds(0)),
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
                    analyzer: "ip_count".to_string(),
                }),
                None,
            ]),
        };

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
                    ttl: 3,
                    analyzer: "ip_count".to_string(),
                }),
            ]),
        };

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
    #[test]
    fn one_ip_provides_ban_only_for_itself() {
        let tc = TestCase {
            input: vec![
                (req_with_ip("1.1.1.1"), Duration::seconds(0)),
                (req_with_ip("2.2.2.2"), Duration::seconds(0)),
                (req_with_ip("3.3.3.3"), Duration::seconds(0)),
                (req_with_ip("3.3.3.3"), Duration::seconds(0)),
                (req_with_ip("1.1.1.1"), Duration::seconds(0)),
            ],
            want_last: None,
            want_every: Some(vec![
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
