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
    let vs = validator::service::Service::build()
        .with_validator(Box::new(validator::dummy::Dummy { idx: 1 }))
        .with_validator(Box::new(validator::dummy::Dummy { idx: 2 }))
        .with_validator(Box::new(validator::dummy::Dummy { idx: 3 }));

    let (s, r) = mpsc::channel(5);
    let (fs, fr) = mpsc::channel::<model::BanRequest>(5);

    tokio::spawn(async move { krs.run(s).await });

    tokio::spawn(async move {
        let fw = forwarder::ExecutorHttpClient::new(cfg.forwarder_url);
        let fw = forwarder::service::Service::new(Box::new(fw));
        fw.run(fr).await
    });

    vs.run(r, fs).await;
    Ok(())
}
