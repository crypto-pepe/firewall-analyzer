use async_trait::async_trait;
use futures::{stream, TryStreamExt};
use kafka::consumer::{Consumer, FetchOffset, GroupOffsetStorage, MessageSet};
use kafka::Error;
use pepe_config::kafka::consumer::Config;
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex};

use crate::consumer::RequestConsumer;
use crate::model::Request;

pub struct KafkaRequestConsumer {
    c: Consumer,
}

impl KafkaRequestConsumer {
    pub fn new(cfg: &Config) -> Result<Self, Error> {
        let mut c = Consumer::from_hosts(cfg.brokers.clone())
            .with_fallback_offset(FetchOffset::Earliest)
            .with_offset_storage(GroupOffsetStorage::Kafka)
            .with_client_id(cfg.client_id.clone())
            .with_group(cfg.group.clone());

        if let Some(t) = cfg.ack_timeout {
            c = c.with_fetch_max_wait_time(t.into());
        }

        c = cfg
            .topics
            .iter()
            .fold(c, |c, t| c.with_topic(t.to_string()));

        let c = c.create()?;
        Ok(KafkaRequestConsumer { c })
    }
}

#[async_trait]
impl RequestConsumer for KafkaRequestConsumer {
    async fn run(&mut self, out: mpsc::Sender<Request>) -> anyhow::Result<()> {
        loop {
            let consumer = Arc::new(Mutex::new(&mut self.c));
            let consumer = consumer.clone();
            let mss = match consumer.lock().await.poll() {
                Ok(mss) => mss,
                Err(e) => {
                    tracing::error!("{:?}", e);
                    continue;
                }
            };

            let stream = stream::iter(mss.iter().map::<Result<MessageSet, anyhow::Error>, _>(Ok));

            stream
                .try_for_each(|ms| async {
                    let fs = ms
                        .messages()
                        .iter()
                        .filter_map(|m| match serde_json::from_slice::<Vec<Request>>(m.value) {
                            Ok(r) => Some(r),
                            Err(e) => {
                                tracing::error!("{:?}", e);
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
            if let Err(e) = consumer.lock().await.commit_consumed() {
                tracing::error!("{:?}", e);
            };
        }
    }
}
