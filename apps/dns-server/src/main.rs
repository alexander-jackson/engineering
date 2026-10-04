use std::collections::HashSet;

use color_eyre::eyre::Result;
use foundation_shutdown::ShutdownCoordinator;
use foundation_templating::TemplateEngine;
use tokio::net::TcpListener;

mod blocklist;
mod cache;
mod config;
mod handler;
mod http_server;
mod persistence;
mod remote_blocklist;
mod server;
mod templates;
mod upstream;

use crate::blocklist::{BlocklistManager, PostgresBlocklistBackend};
use crate::cache::ResponseCache;
use crate::config::Configuration;
use crate::server::{DnsServer, DnsServerMetrics};
use crate::upstream::UpstreamResolver;

#[tokio::main]
async fn main() -> Result<()> {
    let (config, pool) = foundation_init::run_with_bootstrap::<Configuration>().await?;

    tracing::info!(
        upstream = %config.upstream.resolver,
        "dns server initialized"
    );

    let backend = PostgresBlocklistBackend::new(pool.clone());

    let remote_domains = match &config.remote_blocklist {
        Some(remote_config) => remote_blocklist::fetch(&remote_config.url).await?,
        None => HashSet::new(),
    };

    let blocklist_manager = BlocklistManager::new(backend.clone(), remote_domains).await?;
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

    let template_engine = TemplateEngine::new()?;
    let http_server =
        crate::http_server::build(blocklist_manager.clone(), template_engine, http_listener);

    ShutdownCoordinator::new()
        .with_task(dns_server)
        .with_task(http_server)
        .run()
        .await?;

    Ok(())
}
