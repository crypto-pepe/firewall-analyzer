use thiserror::Error;
use tokio::sync::mpsc::error::{SendError, TryRecvError};

#[derive(Error, Debug)]
pub enum ProcessingError<T> {
    #[error(transparent)]
    ChannelUnavailable(#[from] SendError<T>),
    #[error(transparent)]
    ChannelDisconnected(#[from] TryRecvError),
}
