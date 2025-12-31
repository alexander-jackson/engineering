use std::borrow::Cow;

use color_eyre::eyre::Result;
use opentelemetry::Value;
use opentelemetry::trace::TracerProvider;
use opentelemetry_otlp::{Protocol, SpanExporter, WithExportConfig};
use opentelemetry_sdk::Resource;
use opentelemetry_sdk::trace::{SdkTracer, SdkTracerProvider};
use serde::Deserialize;
use tracing_core::Subscriber;
use tracing_opentelemetry::OpenTelemetryLayer;
use tracing_subscriber::Registry;
use tracing_subscriber::registry::LookupSpan;
use tracing_subscriber::reload::{Handle, Layer};

#[derive(Clone, Debug, Deserialize)]
pub struct TelemetryConfig {
    pub enabled: bool,
    pub endpoint: String,
}

/// Handle for reloading the telemetry layer at runtime
pub type ReloadHandle = Handle<Option<OpenTelemetryLayer<Registry, SdkTracer>>, Registry>;

/// Creates a reloadable telemetry layer (initialised as `None`) and returns both the layer and a
/// handle to reload it later. This allows tracing to be initialized early before configuration is
/// loaded, then telemetry can be enabled conditionally.
///
/// # Example
/// ```
/// use std::error::Error;
///
/// use foundation_telemetry::get_trace_layer;
/// use tracing_subscriber::Registry;
/// use tracing_subscriber::layer::SubscriberExt;
///
/// fn main() -> Result<(), Box<dyn Error>> {
///     let (telemetry_layer, reload_handle) = foundation_telemetry::get_reloadable_layer();
///     let subscriber = Registry::default().with(telemetry_layer);
///
///     // Later, after deciding whether to enable telemetry
///     let layer = foundation_telemetry::get_trace_layer("my-app", "localhost:4318")?;
///     reload_handle.reload(Some(layer))?;
///
///     Ok(())
/// }
/// ```
pub fn get_reloadable_layer() -> (
    Layer<Option<OpenTelemetryLayer<Registry, SdkTracer>>, Registry>,
    ReloadHandle,
) {
    Layer::new(None::<OpenTelemetryLayer<Registry, SdkTracer>>)
}

/// Initialises an [`OpenTelemetryLayer`] and sets up exporting for the service to the given
/// endpoint.
///
/// Defaults to using HTTP for trace exports, as well as a binary protocol.
///
/// # Examples
/// ```
/// use std::error::Error;
///
/// use foundation_telemetry::get_trace_layer;
/// use tracing_subscriber::layer::SubscriberExt;
/// use tracing_subscriber::util::SubscriberInitExt;
///
/// fn main() -> Result<(), Box<dyn Error>> {
///     let service = "foobar";
///     let endpoint = "localhost:4318";
///
///     let layer = get_trace_layer(service, endpoint)?;
///
///     tracing_subscriber::registry()
///         .with(layer)
///         .init();
///
///     Ok(())
/// }
/// ```
pub fn get_trace_layer<S, L>(service: S, endpoint: &str) -> Result<OpenTelemetryLayer<L, SdkTracer>>
where
    S: Into<Value> + Clone,
    S: Into<Cow<'static, str>>,
    L: Subscriber + for<'span> LookupSpan<'span>,
{
    let exporter = SpanExporter::builder()
        .with_http()
        .with_protocol(Protocol::HttpBinary)
        .with_endpoint(endpoint)
        .build()?;

    let resource = Resource::builder()
        .with_service_name(service.clone())
        .build();

    let provider = SdkTracerProvider::builder()
        .with_resource(resource)
        .with_batch_exporter(exporter)
        .build();

    let layer = OpenTelemetryLayer::new(provider.tracer(service));

    Ok(layer)
}
