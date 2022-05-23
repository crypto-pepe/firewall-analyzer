use std::fmt::Write;
use std::time::Duration;

use async_trait::async_trait;
use kafka::{consumer, Error, producer};
use kafka::client;
use kafka::client::KafkaClient;
use kafka::consumer::{Consumer, FetchOffset, GroupOffsetStorage};
use kafka::producer::Record;
use tokio::sync::mpsc;

use crate::model::Request;
use crate::receiver::{Config, RequestReceiver};

pub struct KafkaRequestReceiver {
    c: Consumer,
}

impl KafkaRequestReceiver {
    pub fn new(cfg: &Config) -> Result<Self, Error> {
        let mut c = Consumer::from_hosts(cfg.kafka_brokers.clone())
            .with_fallback_offset(FetchOffset::Earliest)
            .with_fetch_max_wait_time(Duration::from_secs(cfg.fetch_max_wait_time_secs))
            .with_fetch_min_bytes(cfg.fetch_min_bytes)
            .with_fetch_max_bytes_per_partition(cfg.fetch_max_bytes_per_partition)
            .with_offset_storage(GroupOffsetStorage::Kafka);

        if cfg.client_id.is_some() {
            c = c.with_client_id(cfg.client_id.clone().unwrap());
        }
        if cfg.group.is_some() {
            c = c.with_group(cfg.group.clone().unwrap());
        }

        for topic in &cfg.topics {
            c = c.with_topic(topic.to_string());
        }

        let c = c.create()?;
        Ok(KafkaRequestReceiver { c })
    }
}

#[async_trait]
impl RequestReceiver for KafkaRequestReceiver {
    async fn run(&mut self, out: mpsc::Sender<Request>) {
        loop {
            for ms in self.c.poll().unwrap().iter() {
                for m in ms.messages() {
                    let req: Request = match serde_json::from_slice(m.value) {
                        Ok(r) => r,
                        Err(e) => {
                            tracing::error!("{:?}", e);
                            continue;
                        }
                    };
                    if let Err(e) = out.send(req).await {
                        tracing::error!("{:?}", e);
                        continue;
                    }
                }
                if let Err(e) = self.c.consume_messageset(ms) {
                    tracing::error!("{:?}", e);
                    continue;
                }
            }
            if let Err(e) = self.c.commit_consumed() {
                tracing::error!("{:?}", e);
            }
        }
    }
}
