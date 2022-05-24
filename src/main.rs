use tokio::io;
use tokio::sync::mpsc;

use crate::forwarder::ExecutorClient;
use crate::receiver::{KafkaRequestReceiver, RequestReceiver};

mod config;
mod forwarder;
mod model;
mod receiver;
mod telemetry;
mod validator;

#[tokio::main]
async fn main() -> io::Result<()> {
    tracing::info!("start application");

    let cfg = match config::Config::load() {
        Ok(a) => a,
        Err(e) => panic!("can't read config {:?}", e),
    };

    tracing::info!("config loaded; config={:?}", &cfg);

    let subscriber = telemetry::get_subscriber(&cfg.telemetry);
    telemetry::init_subscriber(subscriber);

    let mut krs = KafkaRequestReceiver::new(&cfg.kafka).expect("kafka request receiver");
    let mut vs = validator::service::Service::build();

    for v in cfg.validators {
        vs = vs.with_validator(Box::new(validator::get_validator(v)));
    }

    let (s, r) = mpsc::channel(5);
    let (fs, fr) = mpsc::channel::<model::BanRequest>(5);

    tokio::spawn(async move { krs.run(s).await });

    let fw = forwarder::ExecutorHttpClient::new(&cfg.forwarder).expect("create forwarder");
    let fw = forwarder::service::Service::new(Box::new(fw));

    tokio::spawn(async move { fw.run(fr).await });

    vs.run(r, fs).await;
    Ok(())
}
