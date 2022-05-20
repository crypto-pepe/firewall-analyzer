use tokio::sync::mpsc;

use crate::receiver::{KafkaRequestReceiver, RequestReceiver};

mod receiver;
mod config;
mod telemetry;
mod validator;
mod model;
mod sender;

#[tokio::main]
fn main() {
    tracing::info!("start application");

    let cfg = match config::Config::load() {
        Ok(a) => a,
        Err(e) => panic!("can't read config {:?}", e),
    };

    tracing::info!("config loaded; config={:?}", &cfg);

    let subscriber = telemetry::get_subscriber(&cfg.telemetry);
    telemetry::init_subscriber(subscriber);

    let mut krs = KafkaRequestReceiver::new(&cfg.kafka).expect("kafka request receiver");

    let (s, r) = mpsc::channel(2048);

    tokio::spawn(async move {
        krs.run(s)
    });



}
