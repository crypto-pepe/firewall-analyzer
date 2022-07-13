use opentelemetry::sdk::trace::Tracer;
use tracing::subscriber::set_global_default;
use tracing::Subscriber;
use tracing_log::LogTracer;
use tracing_subscriber::prelude::__tracing_subscriber_SubscriberExt;
use tracing_subscriber::{fmt, EnvFilter, Registry};

pub mod config;
pub use self::config::Config;

pub fn get_subscriber(cfg: &Config) -> Box<dyn Subscriber + Send + Sync> {
    let env_filter = EnvFilter::from_default_env();
    let fmt_layer = fmt::Layer::default();
    let reg = Registry::default().with(env_filter).with(fmt_layer);

    if cfg.jaeger_endpoint.is_some() {
        let ep = cfg.jaeger_endpoint.as_ref().unwrap();
        let tracer = init_tracer(cfg.svc_name.to_string(), ep.into());
        return Box::new(reg.with(tracing_opentelemetry::layer().with_tracer(tracer)));
    }

    Box::new(reg)
}

pub fn init_subscriber(subscriber: impl Subscriber + Send + Sync) {
    LogTracer::init().expect("Failed to set logger");
    set_global_default(subscriber).expect("Failed to set subscriber");
}

fn init_tracer(svc_name: String, endpoint: String) -> Tracer {
    opentelemetry_jaeger::new_pipeline()
        .with_service_name(svc_name)
        .with_agent_endpoint(endpoint)
        .install_simple()
        .expect("Failed to install tracer")
}
