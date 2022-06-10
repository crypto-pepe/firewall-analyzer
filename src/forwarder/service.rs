use tokio::sync::mpsc::error::TryRecvError;
use tokio::sync::mpsc::Receiver;

use crate::model::ValidatorBanRequest;
use crate::ExecutorClient;

pub struct Service {
    client: Box<dyn ExecutorClient + Send + Sync>,
    analyzer_name: String,
}

impl Service {
    pub fn new(client: Box<dyn ExecutorClient + Send + Sync>, analyzer_name: String) -> Self {
        Self {
            client,
            analyzer_name,
        }
    }

    pub async fn run(&self, mut recv: Receiver<ValidatorBanRequest>) {
        loop {
            match recv.try_recv() {
                Ok(validator_ban_request) => {
                    let analyzer_id = format!(
                        "{}:{}",
                        self.analyzer_name, validator_ban_request.validator_name
                    );
                    if let Err(e) = self
                        .client
                        .ban(validator_ban_request.ban_request, analyzer_id)
                        .await
                    {
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
