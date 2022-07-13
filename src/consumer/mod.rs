use crate::model::Request;
use anyhow::Result;
use async_trait::async_trait;
use tokio::sync::mpsc;

pub use self::service::KafkaRequestConsumer;

mod service;

#[async_trait]
pub trait RequestConsumer {
    async fn run(&mut self, out: mpsc::UnboundedSender<Request>) -> Result<()>;
}
