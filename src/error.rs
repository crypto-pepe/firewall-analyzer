use std::sync::Arc;
use thiserror::Error;
use tokio::sync::mpsc::error::{SendError, TryRecvError};

#[derive(Error, Debug)]
pub enum ProcessingError<T> {
    #[error(transparent)]
    ChannelUnavailable(#[from] SendError<T>),
    #[error(transparent)]
    ChannelDisconnected(#[from] TryRecvError),
}

#[derive(Clone, Debug, Error)]

pub enum Error {
    #[error("Kafka: {0}")]
    Kafka(#[from] Arc<kafka::Error>),

    #[error("ChannelSend: {0}")]
    ChannelSend(String),
}
