use async_trait::async_trait;
use pepe_config::DurationString;
use reqwest::header::CONTENT_TYPE;
use reqwest::StatusCode;
use serde::{Deserialize, Serialize};

use crate::forwarder::{ForwarderError, ANALYZER_HEADER, APPLICATION_JSON};
use crate::model::BanRequest;
use crate::ExecutorClient;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Config {
    url: String,
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
            .map_err(|e| ForwarderError::NewForwarder(e.to_string()))?;
        Ok(ExecutorHttpClient {
            url: cfg.url.clone(),
            client: cli,
        })
    }
}

#[async_trait]
impl ExecutorClient for ExecutorHttpClient {
    #[tracing::instrument(skip(self))]
    async fn ban(&self, br: BanRequest) -> Result<(), ForwarderError> {
        let res = self
            .client
            .post(self.url.clone())
            // BanRequest derives Serialize
            .body(serde_json::to_vec(&br).expect("BanRequest to vec"))
            .header(ANALYZER_HEADER, br.analyzer.as_str())
            .header(CONTENT_TYPE, APPLICATION_JSON)
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
