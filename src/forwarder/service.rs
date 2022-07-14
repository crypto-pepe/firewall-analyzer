use std::iter::Take;
use tokio::sync::mpsc::Receiver;
use tracing::{debug, info};
use valuable::Valuable;

use crate::error::ProcessingError;
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

    pub async fn run(
        &self,
        mut recv: Receiver<ValidatorBanRequest>,
    ) -> Result<(), ProcessingError<ValidatorBanRequest>> {
        info!("starting forwarder");

        loop {
            if let Some(validator_ban_request) = recv.recv().await {
                debug!(
                    log = "send ban request",
                    validator_ban_request = validator_ban_request.as_value()
                );

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
            } else {
                return Err(ProcessingError::<ValidatorBanRequest>::ChannelClosed);
            }
        }
    }
}
