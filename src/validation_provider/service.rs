use futures::future::join_all;
use tokio::sync::mpsc::error::TryRecvError;
use tokio::sync::mpsc::{Receiver, Sender};

use crate::error::ProcessingError;
use crate::model;
use crate::model::BanRequest;
use crate::validation_provider::Validator;

pub struct Service {
    pub validators: Vec<Box<dyn Validator + Sync + Send>>,
    pub analyzer_prefix: String,
}

impl Service {
    pub fn from_validators(
        vv: Vec<Box<dyn Validator + Sync + Send>>,
        analyzer_prefix: String,
    ) -> Self {
        Self {
            validators: vv,
            analyzer_prefix,
        }
    }

    pub async fn run(
        &mut self,
        mut recv: Receiver<model::Request>,
        send: Sender<BanRequest>,
    ) -> Result<(), ProcessingError<BanRequest>> {
        loop {
            let r = match recv.try_recv() {
                Ok(r) => r,
                Err(e) => match e {
                    TryRecvError::Empty => continue,
                    TryRecvError::Disconnected => {
                        tracing::error!("{:?}", e);
                        return Err(ProcessingError::ChannelDisconnected(e));
                    }
                },
            };

            let handles = self.validators.iter_mut().filter_map(|v| {
                let res = v.validate(r.clone());
                match res {
                    Ok(obr) => match obr {
                        Some(validator_ban_request) => {
                            let ban_request = BanRequest {
                                validator_ban_request,
                                analyzer: format!("{}:{}", self.analyzer_prefix, v.name()),
                            };
                            println!("{}", serde_json::to_string(&ban_request).unwrap());
                            tracing::info!("ban: {:?}", ban_request);
                            Some(send.send(ban_request))
                        }
                        None => None,
                    },
                    Err(e) => {
                        tracing::error!("{:?}", e);
                        None
                    }
                }
            });
            join_all(handles)
                .await
                .into_iter()
                .collect::<Result<(), _>>()
                .map_err(|e| {
                    tracing::error!("{:?}", e);
                    ProcessingError::ChannelUnavailable(e)
                })?;
        }
    }
}
