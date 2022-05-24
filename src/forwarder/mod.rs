use async_trait::async_trait;
use reqwest::StatusCode;
use thiserror::Error;

pub use http_client::ExecutorHttpClient;
pub use noop_client::NoopClient;
pub use service::Service;

use crate::model::BanRequest;

pub mod http_client;
pub mod service;
pub mod noop_client;

pub const ANALYZER_HEADER: &str = "X-Analyzer-Id";
pub const APPLICATION_JSON: &str = "application/json";

#[async_trait]
pub trait ExecutorClient {
    async fn send_ban_request(&self, br: BanRequest) -> Result<(), ForwarderError>;
}

#[derive(Error, Debug)]
pub enum ForwarderError {
    #[error("status code '{0:?}'; body = {1:?}")]
    ResponseNotOk(StatusCode, String),

    #[error("send request error: {0:?}")]
    SendRequest(String),

    #[error("new forwarder error: {0:?}")]
    NewForwarder(String),
}
