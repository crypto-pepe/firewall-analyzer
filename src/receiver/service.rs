use async_trait::async_trait;
use kafka::consumer::{Consumer, FetchOffset, GroupOffsetStorage};
use kafka::Error;
use pepe_config::kafka::consumer::Config;
use tokio::sync::mpsc;

use crate::model::Request;
use crate::receiver::RequestReceiver;

pub struct KafkaRequestReceiver {
    c: Consumer,
}

impl KafkaRequestReceiver {
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
        Ok(KafkaRequestReceiver { c })
    }
}

#[async_trait]
impl RequestReceiver for KafkaRequestReceiver {
    async fn run(&mut self, out: mpsc::Sender<Request>) {
        loop {
            let mss = match self.c.poll() {
                Ok(mss) => mss,
                Err(e) => {
                    tracing::error!("{:?}", e);
                    continue;
                }
            };
            for ms in mss.iter() {
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
