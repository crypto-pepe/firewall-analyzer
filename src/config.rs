use pepe_config::{ConfigError, FileFormat};
use serde::{Deserialize, Serialize};

use crate::{forwarder, telemetry, validator_provider};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Config {
    pub kafka: pepe_config::kafka::consumer::Config,
    pub forwarder: forwarder::http_client::Config,
    pub telemetry: telemetry::Config,
    pub validators: Vec<validator_provider::Config>,
    #[serde(default)]
    pub dry_run: bool,
}

pub const DEFAULT_CONFIG: &str = include_str!("../config.yaml");

impl Config {
    pub fn load() -> Result<Self, ConfigError> {
        pepe_config::load(DEFAULT_CONFIG, FileFormat::Yaml)
    }
}
