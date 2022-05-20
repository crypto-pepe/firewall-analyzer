use async_trait::async_trait;
use tokio::sync::mpsc;

use crate::model::Request;

pub use self::config::Config;
pub use self::service::KafkaRequestReceiver;

mod config;
mod service;

#[async_trait]
pub trait RequestReceiver {
    async fn run(&mut self, out: mpsc::Sender<Request>);
}