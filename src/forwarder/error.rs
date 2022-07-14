use thiserror::Error;

#[derive(Error, Debug)]
pub enum ForwarderError {
    #[error("send request error: {0:?}")]
    SendRequest(String),

    #[error("build forwarder error: {0:?}")]
    BuildForwarder(String),
}
