use tokio::io;
use tokio::sync::mpsc;

use crate::consumer::{KafkaRequestConsumer, RequestConsumer};
use crate::forwarder::ExecutorClient;
use crate::validator::Validator;

mod config;
mod consumer;
mod error;
mod forwarder;
mod model;
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

    let mut kafka_request_consumer =
        KafkaRequestConsumer::new(&cfg.kafka).expect("kafka request consumer");

    let validators = cfg
        .validators
        .into_iter()
        .map(|v| Box::new(validator::get_validator(v)) as Box<dyn Validator + Sync + Send>)
        .collect();
    let validator_svc = validator::service::Service::from_validators(validators);

    let (s, r) = mpsc::channel(5);
    let (fs, fr) = mpsc::channel::<model::BanRequest>(5);

    let request_consumer_handle = tokio::spawn(async move { kafka_request_consumer.run(s).await });

    let fw: Box<dyn ExecutorClient + Send + Sync> = if cfg.dry_run {
        Box::new(forwarder::NoopClient {})
    } else {
        Box::new(forwarder::ExecutorHttpClient::new(&cfg.forwarder).expect("create forwarder"))
    };

    let forwarder_svc = forwarder::service::Service::new(fw);

    let forwarder_handle = tokio::spawn(async move { forwarder_svc.run(fr).await });

    let validator_handle = tokio::spawn(async move { validator_svc.run(r, fs).await });

    tokio::select! {
        res = request_consumer_handle => {
            if let Err(e) = res {
                tracing::error!("{:?}", e)
            }
        },

        _ = forwarder_handle => (),

        res = validator_handle => {
            if let Err(e) = res {
                tracing::error!("{:?}", e)
            }
        }
    }

    Ok(())
}
