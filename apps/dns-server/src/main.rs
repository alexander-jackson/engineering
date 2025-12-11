use std::net::SocketAddr;
use std::time::Duration;

use color_eyre::eyre::Result;
use foundation_init::Configuration;

mod blocklist;
mod cache;
mod config;
mod handler;
mod server;
mod upstream;

use crate::config::ApplicationConfiguration;

#[tokio::main]
async fn main() -> Result<()> {
    let config: Configuration<ApplicationConfiguration> = foundation_init::run()?;

    tracing::info!(
        host = %config.server.host,
        port = config.server.port,
        upstream = %config.upstream.resolver,
        "dns server initialized"
    );

    // Initialize blocklist manager
    let blocklist = crate::blocklist::BlocklistManager::new(config.blocklist.clone());

    // Load initial blocklist
    blocklist.refresh().await?;

    // Spawn background task for periodic blocklist refresh
    let blocklist_clone = blocklist.clone();
    let refresh_interval = Duration::from_secs(config.blocklist.refresh_interval_seconds);
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(refresh_interval);
        loop {
            interval.tick().await;
            if let Err(e) = blocklist_clone.refresh().await {
                tracing::error!(error = ?e, "failed to refresh blocklist");
            }
        }
    });

    // Initialize upstream resolver
    let upstream = crate::upstream::UpstreamResolver::new(&config.upstream).await?;

    // Initialize response cache
    let cache = crate::cache::ResponseCache::new(&config.cache);

    // Build and run DNS server
    let addr = SocketAddr::new(config.server.host.into(), config.server.port);
    let server = crate::server::build(upstream, blocklist, cache, addr).await?;

    server.run().await?;

    Ok(())
}
