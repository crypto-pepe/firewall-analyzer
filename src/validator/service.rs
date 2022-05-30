use anyhow::anyhow;
use tokio::sync::mpsc::{Receiver, Sender};
use tokio::sync::mpsc::error::TryRecvError;

use crate::model;
use crate::validator::Validator;

pub struct Service {
    pub validators: Vec<Box<dyn Validator>>,
}

impl Service {
    pub fn build() -> Self {
        Service {
            validators: Vec::new(),
        }
    }

    pub fn with_validator(mut self, v: Box<dyn Validator>) -> Service {
        self.validators.push(v);
        self
    }

    pub async fn run(&self, mut recv: Receiver<model::Request>, send: Sender<model::BanRequest>) -> Result<(), anyhow::Error> {
        'inf: loop {
            let r = match recv.try_recv() {
                Ok(r) => r,
                Err(e) => match e {
                    TryRecvError::Empty => continue,
                    TryRecvError::Disconnected => {
                        tracing::error!("{:?}", e);
                        return Err(anyhow::Error::from(e));
                    }
                },
            };

            for v in &self.validators {
                match v.validate(r.clone()) {
                    Ok(obr) => match obr {
                        Some(s) => {
                            tracing::info!("ban: {:?}", s);
                            if let Err(e) = send.send(s).await {
                                tracing::error!("{:?}", e);
                                return Err(anyhow::Error::from(e));
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
