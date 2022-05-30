use tokio::sync::mpsc::error::TryRecvError;
use tokio::sync::mpsc::{Receiver, Sender};

use crate::error::ProcessingError;
use crate::model;
use crate::model::BanRequest;
use crate::validator::Validator;

pub struct Service {
    pub validators: Vec<Box<dyn Validator + Sync + Send>>,
}

impl Service {
    pub fn build() -> Self {
        Service {
            validators: Vec::new(),
        }
    }

    pub fn with_validator(mut self, v: Box<dyn Validator + Sync + Send>) -> Service {
        self.validators.push(v);
        self
    }

    pub async fn run(
        &self,
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

            for v in &self.validators {
                let res = v.validate(r.clone());
                match res {
                    Ok(obr) => match obr {
                        Some(s) => {
                            tracing::info!("ban: {:?}", s);
                            if let Err(e) = send.send(s).await {
                                tracing::error!("{:?}", e);
                                return Err(ProcessingError::ChannelUnavailable(e));
                            }
                        }
                        None => (),
                    },
                    Err(e) => {
                        tracing::error!("{:?}", e);
                    }
                }
            }
        }
    }
}
