use std::fmt::format;

use async_trait::async_trait;
use tokio::sync::mpsc::Receiver;

use crate::model::Request;
use crate::sender::Sender;
use crate::validator::Validator;

// Dummy just prints request and returns empty string
pub struct Dummy {
    validators: Vec<dyn Validator>
}

#[async_trait]
impl Sender for Dummy {
    async fn run(&self, recv: Receiver<Request>) {
        for r in recv.recv()
    }
}