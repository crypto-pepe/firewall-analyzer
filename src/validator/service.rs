use tokio::sync::mpsc::{Receiver, Sender};
use tokio::sync::mpsc::error::TryRecvError;

use crate::model;
use crate::validator::Validator;

pub struct Service {
    pub validators: Vec<Box<dyn Validator>>,
}

impl Service {
    pub fn build() -> Self {
        Service { validators: Vec::new() }
    }

    pub fn with_validator(mut self, v: Box<dyn Validator>) -> Service {
        self.validators.push(v);
        self
    }

    pub async fn run(&self, mut recv: Receiver<model::Request>, send: Sender<model::BanRequest>) {
        loop {
            let r = match recv.try_recv() {
                Ok(r) => r,
                Err(e) => match e {
                    TryRecvError::Empty => continue,
                    TryRecvError::Disconnected => {
                        tracing::error!("{:?}", e);
                        return;
                    }
                }
            };

            for v in &self.validators {
                match v.validate(r.clone()) {
                    Ok(obr) => match obr {
                        Some(s) => match send.send(s).await {
                            Err(e) => tracing::error!("{:?}", e),
                            _ => (),
                        }
                        None => ()
                    }
                    Err(e) => {
                        tracing::error!("{:?}", e);
                    }
                }
            }
        }
    }
}

