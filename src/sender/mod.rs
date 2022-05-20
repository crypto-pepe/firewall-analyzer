use async_trait::async_trait;
use tokio::sync::mpsc::Receiver;

use crate::model::Request;

mod dummy;

#[async_trait]
pub trait Sender {
    async fn run(&self, recv: Receiver<Request>);
}