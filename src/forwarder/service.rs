use std::thread::sleep;
use std::time::Duration;

use tokio::sync::mpsc::error::TryRecvError;
use tokio::sync::mpsc::Receiver;

use crate::forwarder::config::Config;
use crate::model::ValidatorBanRequest;
use crate::ExecutorClient;

pub struct Service {
    client: Box<dyn ExecutorClient + Send + Sync>,
    retry_count: usize,
    retry_wait: Duration,
    analyzer_name: String,
}

impl Service {
    pub fn new(
        client: Box<dyn ExecutorClient + Send + Sync>,
        cfg: Config,
        analyzer_name: String,
    ) -> Self {
        Self {
            analyzer_name,
            client,
            retry_count: cfg.retry_count,
            retry_wait: cfg.retry_wait.into(),
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
                        for _ in 1..self.retry_count {
                            sleep(self.retry_wait);
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
