use color_eyre::eyre::Result;
use foundation_shutdown::ShutdownCoordinator;

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

    let blocklist = BlocklistManager::new(config.blocklist.clone()).await?;
    let upstream = UpstreamResolver::new(&config.upstream).await?;
    let cache = ResponseCache::new(&config.cache);

    let tls_config = &config.server.protocols.tls;

    let certificate_resolver = CertificateResolver::new(tls_config.clone()).await?;

    let dns_server = DnsServer::new(
        upstream,
        blocklist,
        cache,
        tls_config,
        certificate_resolver.clone(),
    )
    .await?;

    ShutdownCoordinator::new()
        .with_task(dns_server)
        .with_task(certificate_resolver)
        .run()
        .await?;

    Ok(())
}
