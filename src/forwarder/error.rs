use reqwest::StatusCode;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ForwarderError {
    #[error("status code '{0:?}'; body = {1:?}")]
    ResponseNotOk(StatusCode, String),

    #[error("send request error: {0:?}")]
    SendRequest(String),

    #[error("new forwarder error: {0:?}")]
    NewForwarder(String),
}
