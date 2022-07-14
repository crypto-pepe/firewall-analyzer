mod config;
mod service;

use tokio::sync::mpsc;

pub use self::service::KafkaRequestConsumer;
use crate::error::Error as AppError;
use crate::model::Request;

pub use config::Config;

pub trait RequestConsumer {
    fn run(&mut self, out: mpsc::Sender<Request>) -> Result<(), AppError>;
}
