use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::Duration;

use axum::extract::MatchedPath;
use axum::http::{Request, Response};
use opentelemetry::KeyValue;
use opentelemetry::metrics::Counter;
use opentelemetry_otlp::{MetricExporter, WithExportConfig, WithHttpConfig};
use opentelemetry_sdk::Resource;
use opentelemetry_sdk::metrics::SdkMeterProvider;
use opentelemetry_sdk::metrics::periodic_reader_with_async_runtime::PeriodicReader;
use opentelemetry_sdk::runtime;
use pin_project_lite::pin_project;
use reqwest::Client;
use serde::Deserialize;
use tower_layer::Layer;
use tower_service::Service;

#[derive(Clone, Debug, Deserialize)]
pub struct MetricsConfig {
    pub endpoint: String,
    pub interval_seconds: u64,
}

pub fn init(service_name: &str, config: &MetricsConfig) -> color_eyre::eyre::Result<()> {
    let exporter = MetricExporter::builder()
        .with_http()
        .with_http_client(Client::new())
        .with_endpoint(config.endpoint.clone())
        .build()?;

    let reader = PeriodicReader::builder(exporter, runtime::Tokio)
        .with_interval(Duration::from_secs(config.interval_seconds))
        .build();

    let resource = Resource::builder()
        .with_service_name(service_name.to_owned())
        .build();

    let provider = SdkMeterProvider::builder()
        .with_resource(resource)
        .with_reader(reader)
        .build();

    opentelemetry::global::set_meter_provider(provider);

    Ok(())
}

pub fn http_layer() -> HttpMetricsLayer {
    HttpMetricsLayer::new()
}

#[derive(Clone)]
pub struct HttpMetricsLayer {
    counter: Counter<u64>,
}

impl HttpMetricsLayer {
    fn new() -> Self {
        let meter = opentelemetry::global::meter("http");
        let counter = meter
            .u64_counter("http_requests_total")
            .with_description("Total HTTP responses by path and status code")
            .build();
        Self { counter }
    }
}

impl<S> Layer<S> for HttpMetricsLayer {
    type Service = HttpMetrics<S>;

    fn layer(&self, service: S) -> Self::Service {
        HttpMetrics {
            counter: self.counter.clone(),
            service,
        }
    }
}

#[derive(Clone)]
pub struct HttpMetrics<S> {
    counter: Counter<u64>,
    service: S,
}

impl<S, ReqBody, ResBody> Service<Request<ReqBody>> for HttpMetrics<S>
where
    S: Service<Request<ReqBody>, Response = Response<ResBody>>,
{
    type Response = Response<ResBody>;
    type Error = S::Error;
    type Future = HttpMetricsFuture<S::Future>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.service.poll_ready(cx)
    }

    fn call(&mut self, request: Request<ReqBody>) -> Self::Future {
        let path = request
            .extensions()
            .get::<MatchedPath>()
            .map(|p| p.as_str().to_string())
            .unwrap_or_else(|| request.uri().path().to_string());

        let future = self.service.call(request);

        HttpMetricsFuture {
            inner: future,
            counter: self.counter.clone(),
            path,
        }
    }
}

pin_project! {
    pub struct HttpMetricsFuture<F> {
        #[pin]
        inner: F,
        counter: Counter<u64>,
        path: String,
    }
}

impl<F, B, E> Future for HttpMetricsFuture<F>
where
    F: Future<Output = Result<Response<B>, E>>,
{
    type Output = F::Output;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.project();

        match this.inner.poll(cx) {
            Poll::Ready(Ok(response)) => {
                let status = response.status().as_u16().to_string();
                this.counter.add(
                    1,
                    &[
                        KeyValue::new("path", this.path.clone()),
                        KeyValue::new("status_code", status),
                    ],
                );
                Poll::Ready(Ok(response))
            }
            Poll::Ready(Err(e)) => Poll::Ready(Err(e)),
            Poll::Pending => Poll::Pending,
        }
    }
}
