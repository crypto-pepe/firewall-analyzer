use crate::model::Request;
use anyhow::Result;
use async_trait::async_trait;
use tokio::sync::mpsc;

pub use self::service::KafkaRequestConsumer;

mod config;
mod service;

pub use config::Config;

#[async_trait]
pub trait RequestConsumer {
    async fn run(&mut self, out: mpsc::Sender<Request>) -> Result<()>;
}
