use async_trait::async_trait;
use tokio::sync::mpsc;

use crate::model::Request;

pub use self::service::KafkaRequestReceiver;

mod service;

#[async_trait]
pub trait RequestReceiver {
    async fn run(&mut self, out: mpsc::Sender<Request>);
}
