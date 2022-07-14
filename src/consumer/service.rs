use std::sync::Arc;
use std::thread::sleep;
use std::time::Duration;

use anyhow::Result;
use kafka::consumer::{Consumer, FetchOffset, GroupOffsetStorage};
use kafka::Error;
use tokio::sync::mpsc;
use tracing::{debug, error, info};

use super::Config;
use crate::consumer::RequestConsumer;
use crate::error::Error as AppError;
use crate::model::Request;

pub struct KafkaRequestConsumer {
    consumer: Consumer,
    consuming_delay: Duration,
}

impl KafkaRequestConsumer {
    pub fn new(cfg: &Config) -> Result<Self, Error> {
        let mut consumer = Consumer::from_hosts(cfg.kafka.brokers.clone())
            .with_fallback_offset(FetchOffset::Latest)
            .with_offset_storage(GroupOffsetStorage::Kafka)
            .with_client_id(cfg.kafka.client_id.clone())
            .with_group(cfg.kafka.group.clone());

        if let Some(t) = cfg.kafka.ack_timeout {
            consumer = consumer.with_fetch_max_wait_time(t.into());
        }

        consumer = cfg
            .kafka
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

impl RequestConsumer for KafkaRequestConsumer {
    fn run(&mut self, out: mpsc::Sender<Request>) -> Result<(), AppError> {
        info!("starting kafka consuming");

        loop {
            debug!("fetching message sets");

            let mss = match self.consumer.poll() {
                Ok(mss) => mss,
                Err(e) => {
                    error!("failed to fetch message sets: {:?}", e);
                    continue;
                }
            };

            if mss.is_empty() {
                debug!(
                    "there are no new message sets, sleep for {}s before next poll",
                    self.consuming_delay.as_secs()
                );
                sleep(self.consuming_delay);
            } else {
                debug!("fetched some message sets");

                mss.iter().try_for_each(|ms| {
                    let messages = ms.messages();

                    messages
                        .iter()
                        .filter_map(|m| match serde_json::from_slice::<Vec<Request>>(m.value) {
                            Ok(r) => Some(r),
                            Err(e) => {
                                tracing::error!("failed to deserialize requests: {:?}", e);
                                None
                            }
                        })
                        .flatten()
                        .map(|req| out.blocking_send(req))
                        .collect::<Result<Vec<_>, _>>()
                        .map_err(|e| AppError::ChannelSend(e.to_string()))?;

                    debug!("handled {} messages", messages.len());

                    self.consumer
                        .consume_messageset(ms)
                        .map_err(|e| AppError::Kafka(Arc::new(e)))?;

                    debug!("consumed message set");

                    Result::<(), AppError>::Ok(())
                })?;

                debug!("commiting consumed");

                if let Err(e) = self.consumer.commit_consumed() {
                    tracing::error!("failed to commit consumed: {:?}", e);
                };
            }
        }
    }
}
