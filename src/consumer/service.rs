use crate::consumer::RequestConsumer;
use crate::model::Request;

use anyhow::Result;
use async_trait::async_trait;
use kafka::consumer::{Consumer, FetchOffset, GroupOffsetStorage};
use kafka::Error;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{mpsc, Mutex};
use tracing::{debug, info};

use super::config::Config;

pub struct KafkaRequestConsumer {
    consumer: Arc<Mutex<Consumer>>,
    consuming_delay: Duration,
}

impl KafkaRequestConsumer {
    pub fn new(cfg: &Config) -> Result<Self, Error> {
        let mut consumer = Consumer::from_hosts(cfg.consumer.brokers.clone())
            .with_fallback_offset(FetchOffset::Latest)
            .with_offset_storage(GroupOffsetStorage::Kafka)
            .with_client_id(cfg.consumer.client_id.clone())
            .with_group(cfg.consumer.group.clone());

        if let Some(t) = cfg.consumer.ack_timeout {
            consumer = consumer.with_fetch_max_wait_time(t.into());
        }

        consumer = cfg
            .consumer
            .topics
            .iter()
            .fold(consumer, |c, t| c.with_topic(t.to_string()));

        let consumer = consumer.create()?;
        Ok(Self {
            consumer: Arc::new(Mutex::new(consumer)),
            consuming_delay: cfg.consuming_delay.into(),
        })
    }
}

#[async_trait]
impl RequestConsumer for KafkaRequestConsumer {
    async fn run(&mut self, out: mpsc::Sender<Request>) -> Result<()> {
        info!("starting fetching updates from kafka");

        loop {
            let consumer = self.consumer.clone();

            debug!("fetching messagesets");

            let mss = match consumer.lock().await.poll() {
                Ok(mss) => mss,
                Err(e) => {
                    tracing::error!("failed to poll: {:?}", e);
                    continue;
                }
            };

            let mut consumer = consumer.lock().await;

            mss.iter().try_for_each(|ms| {
                ms.messages()
                    .iter()
                    .filter_map(|m| match serde_json::from_slice::<Vec<Request>>(m.value) {
                        Ok(r) => Some(r),
                        Err(e) => {
                            tracing::error!("failed to deserialize requests: {:?}", e);
                            None
                        }
                    })
                    .flatten()
                    .map(|req| out.blocking_send(req).map_err(|e| anyhow::anyhow!(e)))
                    .collect::<Result<Vec<_>, anyhow::Error>>()?;

                consumer
                    .consume_messageset(ms)
                    .map_err(|e| anyhow::anyhow!(e))
            })?;

            debug!("commiting consumed");

            if let Err(e) = consumer.commit_consumed() {
                tracing::error!("failed to commit consumed: {:?}", e);
            };

            debug!(
                "messagesets sucessfully consumed, sleep for {:?}",
                self.consuming_delay
            );

            tokio::time::sleep(self.consuming_delay).await;

            debug!("sleeped for {:?}", self.consuming_delay);
        }
    }
}
