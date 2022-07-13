use std::iter::Take;

use tokio::sync::mpsc::error::TryRecvError;
use tokio::sync::mpsc::Receiver;
use tracing::info;

use crate::forwarder::config::Config;
use crate::model::ValidatorBanRequest;
use crate::ExecutorClient;

pub struct Service {
    executor: Box<dyn ExecutorClient + Send + Sync>,
    retry_strategy: Take<tokio_retry::strategy::FixedInterval>,
    analyzer_id: String,
}

impl Service {
    pub fn new(
        executor: Box<dyn ExecutorClient + Send + Sync>,
        cfg: Config,
        analyzer_id: String,
    ) -> Self {
        Self {
            analyzer_id,
            executor,
            retry_strategy: tokio_retry::strategy::FixedInterval::new(cfg.retry_interval.into())
                .take(cfg.retry_count),
        }
    }

    pub async fn run(&self, mut recv: Receiver<ValidatorBanRequest>) {
        loop {
            match recv.try_recv() {
                Ok(validator_ban_request) => {
                    info!("emit ban request: {:?}", validator_ban_request);

                    let analyzer_id = format!(
                        "{}:{}",
                        self.analyzer_id, validator_ban_request.validator_name
                    );

                    if let Err(e) = tokio_retry::Retry::spawn(self.retry_strategy.clone(), || {
                        self.executor.ban(
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
