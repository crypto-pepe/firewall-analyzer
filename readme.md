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
| validators               | []object |         | Yes      | List of validator configs. See **Validators**                                                                    |
| dry_run                  | bool     | false   | No       | Run firewall-analyzer in dry run mode                                                                            |

# Validators

### Dummy (testing validator)

#### Config

| Name    | Type   | Default | Required | Note                                   |
|---------|--------|---------|----------|----------------------------------------|
| idx     | int    |         | Yes      | id of validator                        |
| ban_ttl | string | 120s    | No       | TTL for banned target. Duration string |

## Writing your own validator

Inside of `src/validator/` create module with your validator and implement `Validator` trait from `src/validator/mod.rs`.

Inside of `src/validator/mod.rs` add your validator and its parameters to `Config` enum

```rust
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Config {
    Dummy(dummy::Config),
}
```

Then add creating of your validator to `get_validator`

```rust
pub fn get_validator(cfg: Config) -> Box<dyn Validator + Sync + Send> {
    Box::new(match cfg {
        Config::Dummy(cfg) => DummyValidator::new(cfg),
    })
}
```

___

Each of the configuration parameter can be overridden via the environment variable. Nested values overriding are
supported via the '.' separator.

Example:

| Parameter name | Env. variable |
|----------------|---------------|
| some_field     | SOME_FIELD    |
| server.port    | SERVER.PORT   |
