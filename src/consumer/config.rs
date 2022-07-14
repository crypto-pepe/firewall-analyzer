use pepe_config::DurationString;
use serde::Deserialize;

fn default_consuming_delay() -> DurationString {
    DurationString::from_string("1s".to_string()).expect("failed to init DurationString")
}

#[derive(Clone, Debug, Deserialize)]
pub struct Config {
    #[serde(flatten)]
    pub kafka: pepe_config::kafka::consumer::Config,
    #[serde(default = "default_consuming_delay")]
    pub consuming_delay: DurationString,
}
