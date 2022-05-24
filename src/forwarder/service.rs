use tokio::sync::mpsc::error::TryRecvError;
use tokio::sync::mpsc::Receiver;

use crate::model::BanRequest;
use crate::ExecutorClient;

pub struct Service {
    c: Box<dyn ExecutorClient + Send + Sync>,
}

impl Service {
    pub fn new(c: Box<dyn ExecutorClient + Send + Sync>) -> Self {
        Service { c }
    }

    pub async fn run(&self, mut recv: Receiver<BanRequest>) {
        loop {
            match recv.try_recv() {
                // todo maybe blocking receive and inside of select?
                Ok(s) => {
                    if let Err(e) = self.c.send_ban_request(s).await {
                        tracing::error!("{:?}", e)
                    }
                }
                Err(e) => match e {
                    TryRecvError::Empty => (),
                    TryRecvError::Disconnected => break,
                },
            }
        }
    }
}
