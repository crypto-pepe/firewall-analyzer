use async_trait::async_trait;
use futures::future::join_all;
use futures::TryStreamExt;
use kafka::consumer::{Consumer, FetchOffset, GroupOffsetStorage, Message};
use kafka::Error;
use pepe_config::kafka::consumer::Config;
use tokio::sync::mpsc;
use tokio::sync::mpsc::error::SendError;

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

        for topic in &cfg.topics {
            c = c.with_topic(topic.to_string());
        }

        let c = c.create()?;
        Ok(KafkaRequestConsumer { c })
    }
}

#[async_trait]
impl RequestConsumer for KafkaRequestConsumer {
    async fn run(&mut self, out: mpsc::Sender<Request>) -> Result<(), anyhow::Error> {
        loop {
            let mss = match self.c.poll() {
                Ok(mss) => mss,
                Err(e) => {
                    tracing::error!("{:?}", e);
                    continue;
                }
            };
            for ms in mss.iter() {
                let futs = ms
                    .messages()
                    .iter()
                    .filter_map(move |m| {
                        let data = m.value.clone();
                        let reqs: Vec<Request> = match serde_json::from_slice(data) {
                            Ok(r) => r,
                            Err(e) => {
                                tracing::error!("{:?}", e);
                                return None;
                            }
                        };
                        let req_handlers = reqs
                            .into_iter()
                            .filter_map(|req| Some(out.send(req)))
                            .collect::<Vec<_>>();
                        Some(req_handlers)
                    })
                    .flatten();
                let futs = join_all(futs).await;

                // if let Err(e) = futs.iter().try_for_each(|&f| f){
                //   return Err(anyhow::Error::from(e));
                // }

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
