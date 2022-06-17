use std::iter::Take;

use tokio::sync::mpsc::error::TryRecvError;
use tokio::sync::mpsc::Receiver;

use crate::forwarder::config::Config;
use crate::model::ValidatorBanRequest;
use crate::ExecutorClient;

pub struct Service {
    client: Box<dyn ExecutorClient + Send + Sync>,
    retry_strategy: Take<tokio_retry::strategy::FixedInterval>,
    analyzer_id: String,
}

impl Service {
    pub fn new(
        client: Box<dyn ExecutorClient + Send + Sync>,
        cfg: Config,
        analyzer_name: String,
    ) -> Self {
        Self {
            analyzer_id: analyzer_name,
            client,
            retry_strategy: tokio_retry::strategy::FixedInterval::new(cfg.retry_wait.into())
                .take(cfg.retry_count),
        }
    }

    pub async fn run(&self, mut recv: Receiver<ValidatorBanRequest>) {
        loop {
            match recv.try_recv() {
                Ok(validator_ban_request) => {
                    let analyzer_id = format!(
                        "{}:{}",
                        self.analyzer_id, validator_ban_request.validator_name
                    );

                    if let Err(e) = tokio_retry::Retry::spawn(self.retry_strategy.clone(), || {
                        self.client.ban(
                            validator_ban_request.ban_request.clone(),
                            analyzer_id.clone(),
                        )
                    })
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
