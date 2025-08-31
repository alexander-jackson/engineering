use tracing::level_filters::LevelFilter;
use tracing_error::ErrorLayer;
use tracing_subscriber::fmt::Layer;
use tracing_subscriber::layer::{Layered, SubscriberExt};
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{EnvFilter, Registry};

type RegistryLayer = Layered<Layer<Registry>, Registry>;

pub fn install_default_registry() {
    get_default_registry().init();
}

pub fn get_default_registry()
-> Layered<EnvFilter, Layered<ErrorLayer<RegistryLayer>, RegistryLayer>> {
    let fmt_layer = tracing_subscriber::fmt::layer();
    let error_layer = ErrorLayer::default();
    let env_filter_layer = EnvFilter::builder()
        .with_default_directive(LevelFilter::INFO.into())
        .from_env()
        .unwrap_or_else(|_| EnvFilter::new("info"));

    tracing_subscriber::registry()
        .with(fmt_layer)
        .with(error_layer)
        .with(env_filter_layer)
}
