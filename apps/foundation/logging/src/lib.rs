use tracing::level_filters::LevelFilter;
use tracing_error::ErrorLayer;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::registry::LookupSpan;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{EnvFilter, Layer, Registry};

fn get_env_filter() -> EnvFilter {
    EnvFilter::builder()
        .with_default_directive(LevelFilter::INFO.into())
        .from_env()
        .unwrap_or_else(|_| EnvFilter::new("info"))
}

fn add_logging_layers<S>(registry: S) -> impl SubscriberExt
where
    S: SubscriberExt + for<'a> LookupSpan<'a> + Send + Sync + 'static,
{
    registry
        .with(tracing_subscriber::fmt::layer())
        .with(ErrorLayer::default())
        .with(get_env_filter())
}

pub fn install_default_registry() {
    add_logging_layers(Registry::default()).init();
}

pub fn get_registry_with_telemetry<L>(telemetry_layer: L) -> impl SubscriberExt
where
    L: Layer<Registry> + Send + Sync + 'static,
{
    let registry = Registry::default().with(telemetry_layer);
    add_logging_layers(registry)
}
