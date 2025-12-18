use color_eyre::eyre::Result;

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
use crate::upstream::UpstreamResolver;

#[tokio::main]
async fn main() -> Result<()> {
    let config = foundation_init::run::<Configuration>()?;

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
    let upstream = UpstreamResolver::new(&config.upstream).await?;
    let cache = ResponseCache::new(&config.cache);

    let server = DnsServer::new(upstream, blocklist, cache, &config.server.protocols).await?;

    server.run().await?;

    Ok(())
}
