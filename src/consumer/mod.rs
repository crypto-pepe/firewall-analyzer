use async_trait::async_trait;
use tokio::sync::mpsc;

use crate::model::Request;

pub use self::service::KafkaRequestConsumer;

mod service;

#[async_trait]
pub trait RequestConsumer {
    async fn run(&mut self, out: mpsc::Sender<Request>) -> Result<(), anyhow::Error>;
}
