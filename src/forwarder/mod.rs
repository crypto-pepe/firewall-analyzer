use async_trait::async_trait;
use reqwest::StatusCode;

pub use http_client::ExecutorHttpClient;
pub use noop_client::NoopClient;
pub use service::Service;
use crate::forwarder::error::ForwarderError;

use crate::model::BanRequest;

pub mod http_client;
pub mod noop_client;
pub mod service;
pub mod error;

pub const ANALYZER_HEADER: &str = "X-Analyzer-Id";
pub const APPLICATION_JSON: &str = "application/json";

#[async_trait]
pub trait ExecutorClient {
    async fn ban(&self, br: BanRequest) -> Result<(), ForwarderError>;
}

