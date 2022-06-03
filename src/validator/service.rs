use futures::future::join_all;
use tokio::sync::mpsc::error::TryRecvError;
use tokio::sync::mpsc::{Receiver, Sender};

use crate::error::ProcessingError;
use crate::model;
use crate::model::BanRequest;
use crate::validator::generic_validator::{BanRuleConfig, IPReqCountValidator};
use crate::validator::Validator;

pub struct Service {
    pub validators: Vec<Box<dyn Validator + Sync + Send>>,
}

impl Service {
    pub fn from_validators(vv: Vec<Box<dyn Validator + Sync + Send>>) -> Service {
        Service { validators: vv }
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
                        Some(s) => {
                            tracing::info!("ban: {:?}", s);
                            Some(send.send(s))
                        }
                        None => None,
                    },
                    Err(e) => {
                        tracing::error!("{:?}", e);
                        None
                    }
                }
            });
            if let Err(e) = join_all(handles)
                .await
                .into_iter()
                .collect::<Result<(), _>>()
            {
                tracing::error!("{:?}", e);
                return Err(ProcessingError::ChannelUnavailable(e));
            };
        }
    }
}
