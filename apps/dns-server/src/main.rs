use std::time::Duration;

use color_eyre::eyre::Result;
use foundation_recurring_job::RecurringJob;
use foundation_shutdown::ShutdownCoordinator;
use tokio::net::TcpListener;

mod blocklist;
mod cache;
mod config;
mod handler;
mod http_server;
mod persistence;
mod server;
mod upstream;

use crate::blocklist::{BlocklistManager, PostgresBlocklistResolver};
use crate::cache::ResponseCache;
use crate::config::Configuration;
use crate::server::{DnsServer, DnsServerMetrics};
use crate::upstream::UpstreamResolver;

#[tokio::main]
async fn main() -> Result<()> {
    let (config, pool) = foundation_init::run_with_bootstrap::<Configuration>().await?;
    let _ = rustls::crypto::ring::default_provider().install_default();

    tracing::info!(
        upstream = %config.upstream.resolver,
        "dns server initialized"
    );

    let interval = Duration::from_secs(config.blocklist.refresh_interval_seconds);
    let resolver = PostgresBlocklistResolver::new(pool.clone());

    let blocklist_manager = BlocklistManager::new(interval, resolver).await?;
    let upstream = UpstreamResolver::new(&config.upstream).await?;
    let cache = ResponseCache::new(&config.cache);

    let addr = (config.server.dns.host, config.server.dns.port);
    let dns_listener = TcpListener::bind(addr).await?;

    let addr = (config.server.http.host, config.server.http.port);
    let http_listener = TcpListener::bind(addr).await?;

    let meter = opentelemetry::global::meter("dns-server");
    let metrics = DnsServerMetrics::new(&meter);

    let dns_server = DnsServer::new(
        dns_listener,
        upstream,
        blocklist_manager.clone(),
        cache,
        metrics,
    )
    .await?;

    let http_server = crate::http_server::build(blocklist_manager.clone(), http_listener);

    ShutdownCoordinator::new()
        .with_task(dns_server)
        .with_task(http_server)
        .with_task(RecurringJob::new(blocklist_manager))
        .run()
        .await?;

    Ok(())
}
