use std::iter::Take;
use std::thread::sleep;

use tokio::sync::mpsc::error::TryRecvError;
use tokio::sync::mpsc::Receiver;
use tokio_retry::strategy::FixedInterval;

use crate::forwarder::config::Config;
use crate::model::ValidatorBanRequest;
use crate::ExecutorClient;

pub struct Service {
    client: Box<dyn ExecutorClient + Send + Sync>,
    retry_strategy: Take<FixedInterval>,
    analyzer_name: String,
}

impl Service {
    pub fn new(client: Box<dyn ExecutorClient + Send + Sync>, cfg: Config) -> Self {
        Self {
            analyzer_name: cfg.analyzer_name,
            client,
            retry_strategy: FixedInterval::new(cfg.retry_wait.into()).take(cfg.retry_count),
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

                    let mut ban_result = self
                        .client
                        .ban(
                            validator_ban_request.ban_request.clone(),
                            analyzer_id.clone(),
                        )
                        .await;
                    if ban_result.is_err() {
                        for wait in self.retry_strategy.clone() {
                            sleep(wait);
                            ban_result = self
                                .client
                                .ban(
                                    validator_ban_request.ban_request.clone(),
                                    analyzer_id.clone(),
                                )
                                .await;
                            if ban_result.is_ok() {
                                break;
                            }
                        }
                    }
                    if let Err(e) = ban_result {
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
