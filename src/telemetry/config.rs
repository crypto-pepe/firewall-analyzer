use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LogsFormat {
    Full,
    Compact,
    Pretty,
    Json,
}

fn default_format() -> LogsFormat {
    LogsFormat::Full
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Config {
    pub svc_name: String,
    #[serde(default = "default_format")]
    pub format: LogsFormat,
    pub jaeger_endpoint: Option<String>,
}
