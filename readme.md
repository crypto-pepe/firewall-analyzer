# Firewall-analyzer

## ENVs

| Name        | Required | Note                                                                     |
|-------------|----------|--------------------------------------------------------------------------|
| RUST_LOG    | No       | Log level. https://docs.rs/env_logger/0.9.0/env_logger/#enabling-logging |
| CONFIG_PATH | No       | Path to the `yaml` formatted config file                                 |

## Config

**If `CONFIG_PATH` is not stated then `./config.yaml` will be used**


| Name                     | Type     | Default | Required | Note                                                                                                             |
|--------------------------|----------|---------|----------|------------------------------------------------------------------------------------------------------------------|
| kafka.brokers            | []string |         | Yes      | List of kafka brokers                                                                                            |
| kafka.topics             | []string |         | Yes      | List of kafka topics with messages to analyze                                                                    |
| kafka.group              | string   |         | Yes      | Kafka group for this app                                                                                         |
| kafka.client_id          | string   |         | Yes      | Kafka client id for this app                                                                                     |
| forwarder.ban_target_url | string   |         | Yes      | Url to endpoint, implementing [this](https://github.com/crypto-pepe/firewall/wiki/Banned-Targets#ban-target) api |
| forwarder.timeout        | string   |         | No       | Timeout for requests to ban url. Duration string                                                                 |
| forwarder.retry_count    | int      |         | Yes      | Count of attempts to send request to executor                                                                    |
| forwarder.retry_interval | string   |         | Yes      | Time to wait before next try sending request to executor. Duration string                                        |
| analyzer_id              | string   |         | Yes      | Prefix that will be added to analyzer id header                                                                  |
| validators               | []object |         | Yes      | List of validator configs. See **Validators**                                                                    |
| dry_run                  | bool     | false   | No       | Run firewall-analyzer in dry run mode                                                                            |                 
___

Each of the configuration parameter can be overridden via the environment variable. Nested values overriding are
supported via the '.' separator.

Example:

| Parameter name | Env. variable |
|----------------|---------------|
| some_field     | SOME_FIELD    |
| server.port    | SERVER.PORT   |


# Validators

### dummy (testing validator)

#### Config

| Name    | Type   | Default | Required | Note                                   |
|---------|--------|---------|----------|----------------------------------------|
| idx     | int    |         | Yes      | id of validator                        |
| ban_ttl | string | 120s    | No       | TTL for banned target. Duration string |

### requests_from_ip_count

#### Config

| Name                  | Type   | Default | Required | Note                                        |
|-----------------------|--------|---------|----------|---------------------------------------------|
| ban_description       | string |         | Yes      | Reason sending to executor                  |
| limits.limit          | int    |         | Yes      | Count of requests allowed on current limit  |
| limits.ban_duration   | string |         | Yes      | TTL for banned target. Duration string      |
| limits.reset_duration | string |         | Yes      | Duration for resetting ban. Duration string |

### requests_from_ua_count
Analyzes only user agents, that matches one of patterns

#### Config

| Name                  | Type     | Default | Required | Note                                        |
|-----------------------|----------|---------|----------|---------------------------------------------|
| ban_description       | string   |         | Yes      | Reason sending to executor                  |
| limits.limit          | int      |         | Yes      | Count of requests allowed on current limit  |
| limits.ban_duration   | string   |         | Yes      | TTL for banned target. Duration string      |
| limits.reset_duration | string   |         | Yes      | Duration for resetting ban. Duration string |
| patterns              | []string |         | Yes      | Regexps for user agent                      |

### requests_from_ip_cost
Bans ip if total requests cost reaches limit. Cost of first matching pattern is chosen. If there is no matching patterns 
then default cost is used. 
#### Config

| Name                  | Type   | Default | Required | Note                                                        |
|-----------------------|--------|---------|----------|-------------------------------------------------------------|
| ban_description       | string |         | Yes      | Reason sending to executor                                  |
| limits.limit          | int    |         | Yes      | Total cost of requests allowed on current limit             |
| limits.ban_duration   | string |         | Yes      | TTL for banned target. Duration string                      |
| limits.reset_duration | string |         | Yes      | Duration for resetting ban. Duration string                 |
| default_cost          | int    |         | Yes      | Default cost of request is used if none of patterns matched |
| patterns.method       | string |         | No       | Method of request. If not stated, any method matches        |
| patterns.path_regex   | string |         | Yes      | Regexp for request's path                                   |
| patterns.body_regex   | string |         | No       | Regexp for request's body. If not stated, any body matches  |
| patterns.cost         | int    |         | Yes      | Cost of matching requests                                   |

## Writing your own validator

Inside of `src/validators/` create module with your validator and implement `Validator` trait from `src/validation_provider/mod.rs`.

Inside of `src/validation_provider/mod.rs` add your validator's config to `Config` enum

```rust
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Config {
    Dummy(dummy::Config),
    MyNewValidator(my_new_validator::Config),
}
```

Then add creating of your validator to `get_validator`

```rust
pub fn get_validator(cfg: Config) -> Box<dyn Validator + Sync + Send> {
    match cfg {
        Config::Dummy(cfg) => Box::new(DummyValidator::new(cfg)),
        Config::MyNewValidator(cfg) => Box::new(MyNewValidator::new(cfg)),
    }
}
```
