use pepe_config::{ConfigError, FileFormat};
use serde::{Deserialize, Serialize};

use crate::{forwarder, telemetry, validation_provider};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Config {
    pub kafka: pepe_config::kafka::consumer::Config,
    pub analyzer_id: String,
    pub forwarder: forwarder::config::Config,
    pub telemetry: telemetry::Config,
    pub validators: Vec<validation_provider::Config>,
    #[serde(default)]
    pub dry_run: bool,
}

pub const DEFAULT_CONFIG: &str = include_str!("../config.yaml");

impl Config {
    pub fn load() -> Result<Self, ConfigError> {
        pepe_config::load(DEFAULT_CONFIG, FileFormat::Yaml)
    }
}
