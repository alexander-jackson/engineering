use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use color_eyre::eyre::Result;
use foundation_shutdown::{CancellationToken, GracefulTask};
use hickory_server::ServerFuture;
use tokio::net::TcpListener;

use crate::blocklist::BlocklistManager;
use crate::cache::ResponseCache;
use crate::config::TlsConfig;
use crate::handler::DnsRequestHandler;
use crate::tls::CertificateResolver;
use crate::upstream::UpstreamResolver;

pub struct DnsServer {
    server_future: ServerFuture<DnsRequestHandler>,
}

impl DnsServer {
    #[tracing::instrument(skip(upstream, blocklist, cache, config, certificate_resolver))]
    pub async fn new(
        upstream: UpstreamResolver,
        blocklist: BlocklistManager,
        cache: ResponseCache,
        config: &TlsConfig,
        certificate_resolver: CertificateResolver,
    ) -> Result<Self> {
        let handler = DnsRequestHandler::new(upstream, blocklist, cache);
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
