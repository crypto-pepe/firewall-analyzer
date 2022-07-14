use futures::future::try_join_all;
use tokio::sync::mpsc::error::TryRecvError;
use tokio::sync::mpsc::{Receiver, Sender};
use tracing::{debug, info};

use crate::error::ProcessingError;
use crate::model;
use crate::model::ValidatorBanRequest;
use crate::validation_provider::Validator;

pub struct Service {
    pub validators: Vec<Box<dyn Validator + Sync + Send>>,
}

impl Service {
    pub fn from_validators(validators: Vec<Box<dyn Validator + Sync + Send>>) -> Self {
        Self { validators }
    }

    pub async fn run(
        &mut self,
        mut request_stream: Receiver<model::Request>,
        ban_sink: Sender<ValidatorBanRequest>,
    ) -> Result<(), ProcessingError<ValidatorBanRequest>> {
        info!("starting validators provider");

        loop {
            let r = match request_stream.try_recv() {
                Ok(r) => {
                    debug!("got request to analyze");
                    r
                }
                Err(e) => match e {
                    TryRecvError::Empty => continue,
                    TryRecvError::Disconnected => {
                        tracing::error!("{:?}", e);
                        return Err(ProcessingError::ChannelDisconnected(e));
                    }
                },
            };

            let fs = self
                .validators
                .iter_mut()
                .filter_map(|v| match v.validate(r.clone()) {
                    Ok(obr) => match obr {
                        Some(validator_ban_request) => {
                            let ban_request = ValidatorBanRequest {
                                ban_request: validator_ban_request,
                                validator_name: v.name(),
                            };
                            tracing::info!("ban: {:?}", ban_request);
                            Some(ban_sink.send(ban_request))
                        }
                        None => None,
                    },
                    Err(e) => {
                        tracing::error!("{:?}", e);
                        None
                    }
                });

            try_join_all(fs).await.map_err(|e| {
                tracing::error!("{:?}", e);
                ProcessingError::ChannelUnavailable(e)
            })?;
        }
    }
}
