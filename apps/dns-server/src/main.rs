use std::time::Duration;

use color_eyre::eyre::Result;
use foundation_recurring_job::RecurringJob;
use foundation_shutdown::ShutdownCoordinator;
use opentelemetry_otlp::{MetricExporter, WithExportConfig, WithHttpConfig};
use opentelemetry_sdk::metrics::SdkMeterProvider;
use opentelemetry_sdk::metrics::periodic_reader_with_async_runtime::PeriodicReader;
use opentelemetry_sdk::runtime;
use reqwest::Client;

mod blocklist;
mod cache;
mod config;
mod handler;
mod server;
mod tls;
mod upstream;

use crate::blocklist::BlocklistManager;
use crate::cache::ResponseCache;
use crate::config::Configuration;
use crate::server::DnsServer;
use crate::tls::CertificateResolver;
use crate::upstream::UpstreamResolver;

#[tokio::main]
async fn main() -> Result<()> {
    let config = foundation_init::run::<Configuration>()?;
    let _ = rustls::crypto::ring::default_provider().install_default();

    tracing::info!(
        upstream = %config.upstream.resolver,
        "dns server initialized"
    );

    let exporter = MetricExporter::builder()
        .with_http()
        .with_http_client(Client::new())
        .with_endpoint(config.metrics.endpoint.clone())
        .build()?;

    let reader = PeriodicReader::builder(exporter, runtime::Tokio)
        .with_interval(Duration::from_secs(config.metrics.interval_seconds))
        .build();
    let provider = SdkMeterProvider::builder().with_reader(reader).build();

    opentelemetry::global::set_meter_provider(provider);

    let blocklist = BlocklistManager::new(config.blocklist.clone()).await?;
    let upstream = UpstreamResolver::new(&config.upstream).await?;
    let cache = ResponseCache::new(&config.cache);

    let tls_config = &config.server.protocols.tls;

    let certificate_resolver = CertificateResolver::new(tls_config.clone()).await?;

    let meter = opentelemetry::global::meter("dns-server");

    let requests = meter
        .u64_counter("dns_requests_total")
        .with_description("Total number of DNS requests received")
        .build();

    let responses = meter
        .u64_counter("dns_responses_total")
        .with_description("Total number of DNS responses sent")
        .build();

    let dns_server = DnsServer::new(
        upstream,
        blocklist.clone(),
        cache,
        tls_config,
        certificate_resolver.clone(),
        requests,
        responses,
    )
    .await?;

    ShutdownCoordinator::new()
        .with_task(dns_server)
        .with_task(RecurringJob::new(blocklist))
        .with_task(RecurringJob::new(certificate_resolver))
        .run()
        .await?;

    Ok(())
}
