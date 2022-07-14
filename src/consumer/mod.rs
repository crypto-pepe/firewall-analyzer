use crate::model::Request;
use tokio::sync::mpsc;

pub use self::service::KafkaRequestConsumer;
use crate::error::Error as AppError;

mod service;

pub trait RequestConsumer {
    fn run(&mut self, out: mpsc::Sender<Request>) -> Result<(), AppError>;
}
