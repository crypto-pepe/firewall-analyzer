use std::sync::Arc;

use anyhow::Result;
use kafka::consumer::{Consumer, FetchOffset, GroupOffsetStorage};
use kafka::Error;
use pepe_config::kafka::consumer::Config;
use tokio::sync::mpsc;

use crate::consumer::RequestConsumer;
use crate::error::Error as AppError;
use crate::model::Request;

pub struct KafkaRequestConsumer {
    consumer: Consumer,
}

impl KafkaRequestConsumer {
    pub fn new(cfg: &Config) -> Result<Self, Error> {
        let mut consumer = Consumer::from_hosts(cfg.brokers.clone())
            .with_fallback_offset(FetchOffset::Earliest)
            .with_offset_storage(GroupOffsetStorage::Kafka)
            .with_client_id(cfg.client_id.clone())
            .with_group(cfg.group.clone());

        if let Some(t) = cfg.ack_timeout {
            consumer = consumer.with_fetch_max_wait_time(t.into());
        }

        consumer = cfg
            .topics
            .iter()
            .fold(consumer, |c, t| c.with_topic(t.to_string()));

        let consumer = consumer.create()?;
        Ok(Self { consumer })
    }
}

impl RequestConsumer for KafkaRequestConsumer {
    fn run(&mut self, out: mpsc::Sender<Request>) -> Result<(), AppError> {
        loop {
            let mss = match self.consumer.poll() {
                Ok(mss) => mss,
                Err(e) => {
                    tracing::error!("{:?}", e);
                    continue;
                }
            };

            mss.iter().try_for_each(|ms| {
                ms.messages()
                    .iter()
                    .filter_map(|m| match serde_json::from_slice::<Vec<Request>>(m.value) {
                        Ok(r) => Some(r),
                        Err(e) => {
                            tracing::error!("{:?}", e);
                            None
                        }
                    })
                    .flatten()
                    .map(|req| out.blocking_send(req))
                    .collect::<Result<Vec<_>, _>>()
                    .map_err(|e| AppError::ChannelSend(e.to_string()))?;

                self.consumer
                    .consume_messageset(ms)
                    .map_err(|e| AppError::Kafka(Arc::new(e)))
            })?;

            if let Err(e) = self.consumer.commit_consumed() {
                tracing::error!("{:?}", e);
            };
        }
    }
}
