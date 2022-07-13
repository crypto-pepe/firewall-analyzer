use crate::consumer::RequestConsumer;
use crate::model::Request;
use anyhow::Result;
use async_trait::async_trait;
use futures::{stream, TryStreamExt};
use kafka::consumer::{Consumer, FetchOffset, GroupOffsetStorage, MessageSet};
use kafka::Error;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{mpsc, Mutex};
use tracing::{debug, info};

use super::config::Config;

pub struct KafkaRequestConsumer {
    consumer: Consumer,
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
            consumer,
            consuming_delay: cfg.consuming_delay.into(),
        })
    }
}

#[async_trait]
impl RequestConsumer for KafkaRequestConsumer {
    async fn run(&mut self, out: mpsc::Sender<Request>) -> Result<()> {
        info!("starting fetching updates from kafka");

        loop {
            let consumer = Arc::new(Mutex::new(&mut self.consumer));
            let consumer = consumer.clone();

            debug!("fetching messagesets");

            let mss = match consumer.lock().await.poll() {
                Ok(mss) => mss,
                Err(e) => {
                    tracing::error!("failed to poll: {:?}", e);
                    continue;
                }
            };

            let stream = stream::iter(mss.iter().map::<Result<MessageSet>, _>(Ok));

            stream
                .try_for_each(|ms| async {
                    let fs = ms
                        .messages()
                        .iter()
                        .filter_map(|m| match serde_json::from_slice::<Vec<Request>>(m.value) {
                            Ok(r) => Some(r),
                            Err(e) => {
                                tracing::error!("failed to deserialize requests: {:?}", e);
                                None
                            }
                        })
                        .flatten()
                        .map(|req| out.send(req))
                        .collect::<Vec<_>>();

                    futures::future::try_join_all(fs).await?;

                    consumer
                        .lock()
                        .await
                        .consume_messageset(ms)
                        .map_err(|e| e.into())
                })
                .await?;

            debug!("messagesets consumed");

            if let Err(e) = consumer.lock().await.commit_consumed() {
                tracing::error!("failed to commit consumed: {:?}", e);
            };

            debug!("messagesets sucessfully consumed");

            tokio::time::sleep(self.consuming_delay).await;
        }
    }
}
