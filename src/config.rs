use std::env;
use std::fs::File;
use std::io::Read;

use config::ConfigError;
use serde::{Deserialize, Serialize};

use crate::receiver;
use crate::telemetry;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Config {
    pub kafka: receiver::Config,
    pub forwarder_urls: Vec<String>,
    pub telemetry: telemetry::Config,
    // todo
    pub validators: Vec<()>,
}


pub const DEFAULT_CONFIG: &str = include_str!("../config.yaml");

impl Config {
    pub fn load() -> Result<Self, ConfigError> {
        pepe_config::load(DEFAULT_CONFIG, config::FileFormat::Yaml)
    }
}
