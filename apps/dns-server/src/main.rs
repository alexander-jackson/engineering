use color_eyre::eyre::Result;
use foundation_recurring_job::RecurringJob;
use foundation_shutdown::ShutdownCoordinator;
use tokio::net::TcpListener;

mod blocklist;
mod cache;
mod config;
mod handler;
mod server;
mod upstream;

use crate::blocklist::BlocklistManager;
use crate::cache::ResponseCache;
use crate::config::Configuration;
use crate::server::{DnsServer, DnsServerMetrics};
use crate::upstream::UpstreamResolver;

#[tokio::main]
async fn main() -> Result<()> {
    let config = foundation_init::run::<Configuration>()?;
    let _ = rustls::crypto::ring::default_provider().install_default();

    tracing::info!(
        upstream = %config.upstream.resolver,
        "dns server initialized"
    );

    let blocklist = BlocklistManager::new(config.blocklist.clone()).await?;
    let upstream = UpstreamResolver::new(&config.upstream).await?;
    let cache = ResponseCache::new(&config.cache);

    let host = config.server.host;
    let port = config.server.port;
    let listener = TcpListener::bind((host, port)).await?;

    let meter = opentelemetry::global::meter("dns-server");
    let metrics = DnsServerMetrics::new(&meter);

    let dns_server = DnsServer::new(listener, upstream, blocklist.clone(), cache, metrics).await?;

    ShutdownCoordinator::new()
        .with_task(dns_server)
        .with_task(RecurringJob::new(blocklist))
        .run()
        .await?;

    Ok(())
}
