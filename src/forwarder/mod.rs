use anyhow::Error;
use async_trait::async_trait;
use reqwest::header::CONTENT_TYPE;
use reqwest::StatusCode;

use crate::model::BanRequest;

pub mod service;

#[async_trait]
pub trait ExecutorClient {
    async fn send_ban_request(&self, br: BanRequest) -> Result<(), Error>;
}

pub struct ExecutorHttpClient {
    url: String,
    cli: reqwest::Client,
}

impl ExecutorHttpClient {
    pub fn new(url: String) -> Self {
        let cli = reqwest::Client::new();
        ExecutorHttpClient { url, cli }
    }
}

const ANALYZER_HEADER: &str = "X-Analyzer-Id";

#[async_trait]
impl ExecutorClient for ExecutorHttpClient {
    // todo
    async fn send_ban_request(&self, br: BanRequest) -> Result<(), Error> {
        let res = self
            .cli
            .post(self.url.clone())
            .body(serde_json::to_vec(&br).expect("ban request to vec"))
            .header(ANALYZER_HEADER, br.analyzer.as_str())
            .header(CONTENT_TYPE, "application/json")
            .send()
            .await?;
        if res.status() != StatusCode::NO_CONTENT {
            return Err(Error::msg("status is not OK"));
        }
        Ok(())
    }
}
