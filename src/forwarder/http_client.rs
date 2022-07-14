use async_trait::async_trait;
use futures::{stream, TryStreamExt};
use pepe_config::DurationString;
use reqwest::StatusCode;
use serde::{Deserialize, Serialize};
use tracing::error;

use crate::forwarder::{ForwarderError, ANALYZER_HEADER};
use crate::model::BanRequest;
use crate::ExecutorClient;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Config {
    ban_target_urls: Vec<String>,
    timeout: Option<DurationString>,
}

pub struct ExecutorHttpClient {
    urls: Vec<String>,
    client: reqwest::Client,
}

impl ExecutorHttpClient {
    pub fn new(cfg: &Config) -> Result<Self, ForwarderError> {
        let mut client = reqwest::Client::builder();
        if let Some(t) = cfg.timeout {
            client = client.timeout(t.into())
        }

        let client = client
            .build()
            .map_err(|e| ForwarderError::BuildForwarder(e.to_string()))?;

        Ok(Self {
            urls: cfg.ban_target_urls.clone(),
            client: client,
        })
    }
}

#[async_trait]
impl ExecutorClient for ExecutorHttpClient {
    #[tracing::instrument(skip(self))]
    async fn ban(&self, br: BanRequest, analyzer_id: String) -> Result<(), ForwarderError> {
        let stream = stream::iter(
            self.urls
                .iter()
                .map(|url| Result::<&String, ForwarderError>::Ok(url)),
        );
        stream
            .try_for_each(|url| {
                let br = br.clone();
                let analyzer_id = analyzer_id.clone();
                async move {
                    let res = self
                        .client
                        .post(url)
                        .json(&br.clone())
                        .header(ANALYZER_HEADER, analyzer_id.clone().as_str())
                        .send()
                        .await
                        .map_err(|e| ForwarderError::SendRequest(e.to_string()))?;

                    if res.status() != StatusCode::NO_CONTENT {
                        let res_status = res.status().as_u16();
                        let res_text = res.text().await.unwrap_or_default();
                        error!(
                            log = "failed while sending ban request to executor",
                            res_status = res_status,
                            res_text = &res_text.as_str()
                        );
                    }

                    Ok(())
                }
            })
            .await?;

        Ok(())
    }
}
