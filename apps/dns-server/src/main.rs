use std::time::Duration;

use color_eyre::eyre::Result;
use foundation_init::Configuration;

mod blocklist;
mod cache;
mod config;
mod handler;
mod server;
mod tls;
mod upstream;

use crate::config::ApplicationConfiguration;

#[tokio::main]
async fn main() -> Result<()> {
    let config: Configuration<ApplicationConfiguration> = foundation_init::run()?;

    // Install rustls crypto provider if TLS is enabled (required for TLS support)
    if config.server.protocols.tls.is_some() {
        let _ = rustls::crypto::ring::default_provider().install_default();
    }

    tracing::info!(
        udp_enabled = config.server.protocols.udp.is_some(),
        tls_enabled = config.server.protocols.tls.is_some(),
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
    let server = crate::server::build(upstream, blocklist, cache, &config.server.protocols).await?;

    server.run().await?;

    Ok(())
}
