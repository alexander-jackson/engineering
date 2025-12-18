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

use crate::blocklist::BlocklistManager;
use crate::cache::ResponseCache;
use crate::config::ApplicationConfiguration;
use crate::upstream::UpstreamResolver;

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

    let blocklist = BlocklistManager::new(config.blocklist.clone()).await?;
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

    let upstream = UpstreamResolver::new(&config.upstream).await?;
    let cache = ResponseCache::new(&config.cache);

    let server = crate::server::build(upstream, blocklist, cache, &config.server.protocols).await?;

    server.run().await?;

    Ok(())
}
