use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use color_eyre::eyre::Result;
use foundation_shutdown::{CancellationToken, GracefulTask};
use hickory_server::ServerFuture;
use opentelemetry::metrics::{Counter, Histogram, Meter};
use tokio::net::TcpListener;

use crate::blocklist::BlocklistManager;
use crate::cache::ResponseCache;
use crate::config::TlsConfig;
use crate::handler::DnsRequestHandler;
use crate::tls::CertificateResolver;
use crate::upstream::UpstreamResolver;

#[derive(Clone)]
pub struct DnsServerMetrics {
    pub(crate) requests: Counter<u64>,
    pub(crate) responses: Counter<u64>,
    pub(crate) request_duration: Histogram<f64>,
    pub(crate) upstream_duration: Histogram<f64>,
}

impl DnsServerMetrics {
    pub fn new(meter: &Meter) -> Self {
        Self {
            requests: meter
                .u64_counter("dns_requests_total")
                .with_description("Total number of DNS requests received")
                .build(),
            responses: meter
                .u64_counter("dns_responses_total")
                .with_description("Total number of DNS responses sent")
                .build(),
            request_duration: meter
                .f64_histogram("dns_request_duration_ms")
                .with_description("End-to-end latency of DNS request handling in milliseconds")
                .build(),
            upstream_duration: meter
                .f64_histogram("dns_upstream_duration_ms")
                .with_description("Latency of upstream DNS resolution in milliseconds")
                .build(),
        }
    }
}

pub struct DnsServer {
    server_future: ServerFuture<DnsRequestHandler>,
}

impl DnsServer {
    #[tracing::instrument(skip(upstream, blocklist, cache, config, certificate_resolver, metrics))]
    pub async fn new(
        upstream: UpstreamResolver,
        blocklist: BlocklistManager,
        cache: ResponseCache,
        config: &TlsConfig,
        certificate_resolver: CertificateResolver,
        metrics: DnsServerMetrics,
    ) -> Result<Self> {
        let handler = DnsRequestHandler::new(upstream, blocklist, cache, metrics);
        let mut server_future = ServerFuture::new(handler);

        register_tls_listener(&mut server_future, config, certificate_resolver).await?;

        Ok(Self { server_future })
    }
}

impl GracefulTask for DnsServer {
    async fn run_until_shutdown(mut self, token: CancellationToken) -> Result<()> {
        tokio::select! {
            result = self.server_future.block_until_done() => {
                result?;
                tracing::info!("dns server stopped normally");
            }
            _ = token.cancelled() => {
                tracing::info!("received shutdown signal, stopping dns server");
            }
        }

        Ok(())
    }
}

async fn register_tls_listener(
    server_future: &mut ServerFuture<DnsRequestHandler>,
    config: &TlsConfig,
    certificate_resolver: CertificateResolver,
) -> Result<()> {
    let addr = SocketAddr::new(config.host.into(), config.port);
    let listener = TcpListener::bind(addr).await?;

    tracing::info!(%addr, "bound TLS listener for DNS over TLS queries");

    let timeout = Duration::from_secs(300);
    server_future.register_tls_listener(listener, timeout, Arc::new(certificate_resolver))?;

    Ok(())
}
