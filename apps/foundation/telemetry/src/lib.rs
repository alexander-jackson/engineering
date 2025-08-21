use std::borrow::Cow;

use color_eyre::eyre::Result;
use opentelemetry::trace::TracerProvider;
use opentelemetry::Value;
use opentelemetry_otlp::{Protocol, SpanExporter, WithExportConfig};
use opentelemetry_sdk::trace::{SdkTracer, SdkTracerProvider};
use opentelemetry_sdk::Resource;
use tracing_core::Subscriber;
use tracing_opentelemetry::OpenTelemetryLayer;
use tracing_subscriber::registry::LookupSpan;

pub fn get_trace_layer<S, L>(service: S, endpoint: &str) -> Result<OpenTelemetryLayer<L, SdkTracer>>
where
    S: Into<Value> + Copy,
    S: Into<Cow<'static, str>>,
    L: Subscriber + for<'span> LookupSpan<'span>,
{
    let exporter = SpanExporter::builder()
        .with_http()
        .with_protocol(Protocol::HttpBinary)
        .with_endpoint(endpoint)
        .build()?;

    let resource = Resource::builder().with_service_name(service).build();

    let provider = SdkTracerProvider::builder()
        .with_resource(resource)
        .with_batch_exporter(exporter)
        .build();

    let layer = OpenTelemetryLayer::new(provider.tracer(service));

    Ok(layer)
}
