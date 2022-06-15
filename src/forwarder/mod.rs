use async_trait::async_trait;

use crate::forwarder::error::ForwarderError;
pub use http_client::ExecutorHttpClient;
pub use noop_client::NoopClient;
pub use service::Service;

use crate::model::BanRequest;

pub mod config;
pub mod error;
pub mod http_client;
pub mod noop_client;
pub mod service;

pub const ANALYZER_HEADER: &str = "X-Analyzer-Id";

#[async_trait]
pub trait ExecutorClient {
    async fn ban(&self, br: BanRequest, analyzer_name: String) -> Result<(), ForwarderError>;
}
