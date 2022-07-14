use async_trait::async_trait;
use pepe_config::DurationString;
use reqwest::StatusCode;
use serde::{Deserialize, Serialize};

use crate::forwarder::{ForwarderError, ANALYZER_HEADER};
use crate::model::BanRequest;
use crate::ExecutorClient;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Config {
    ban_target_url: String,
    timeout: Option<DurationString>,
}

pub struct ExecutorHttpClient {
    url: String,
    client: reqwest::Client,
}

impl ExecutorHttpClient {
    pub fn new(cfg: &Config) -> Result<Self, ForwarderError> {
        let mut cli = reqwest::Client::builder();
        if let Some(t) = cfg.timeout {
            cli = cli.timeout(t.into())
        }
        let cli = cli
            .build()
            .map_err(|e| ForwarderError::BuildForwarder(e.to_string()))?;
        Ok(Self {
            url: cfg.ban_target_url.clone(),
            client: cli,
        })
    }
}

#[async_trait]
impl ExecutorClient for ExecutorHttpClient {
    #[tracing::instrument(skip(self))]
    async fn ban(&self, br: BanRequest, analyzer_id: String) -> Result<(), ForwarderError> {
        let res = self
            .client
            .post(self.url.clone())
            .json(&br)
            .header(ANALYZER_HEADER, analyzer_id.as_str())
            .send()
            .await
            .map_err(|e| ForwarderError::SendRequest(e.to_string()))?;
        if res.status() != StatusCode::NO_CONTENT {
            return Err(ForwarderError::ResponseNotOk(
                res.status(),
                res.text().await.unwrap_or_default(),
            ));
        }
        Ok(())
    }
}
