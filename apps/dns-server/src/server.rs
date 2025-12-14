use std::net::SocketAddr;

use color_eyre::eyre::Result;
use foundation_shutdown::ShutdownCoordinator;
use hickory_server::ServerFuture;
use tokio::net::{TcpListener, UdpSocket};

use crate::blocklist::BlocklistManager;
use crate::cache::ResponseCache;
use crate::config::{ProtocolsConfig, TlsConfig, UdpConfig};
use crate::handler::DnsRequestHandler;
use crate::upstream::UpstreamResolver;

/// DNS server with graceful shutdown support
pub struct DnsServer {
    server_future: ServerFuture<DnsRequestHandler>,
    coordinator: ShutdownCoordinator,
}

impl DnsServer {
    /// Run the DNS server until shutdown signal is received
    #[tracing::instrument(skip(self))]
    pub async fn run(mut self) -> Result<()> {
        let mut receiver = self.coordinator.subscribe();
        let coordinator = self.coordinator;
        tokio::spawn(async move { coordinator.spawn().await });

        tracing::info!("dns server started");

        tokio::select! {
            result = self.server_future.block_until_done() => {
                result?;
                tracing::info!("dns server stopped normally");
            }
            _ = receiver.recv() => {
                tracing::info!("received shutdown signal, stopping dns server");
            }
        }

        Ok(())
    }
}

/// Build a DNS server with UDP and/or TLS support
#[tracing::instrument(skip(upstream, blocklist, cache))]
pub async fn build(
    upstream: UpstreamResolver,
    blocklist: BlocklistManager,
    cache: ResponseCache,
    protocols: &ProtocolsConfig,
) -> Result<DnsServer> {
    // Create the request handler (shared across all protocols)
    let handler = DnsRequestHandler::new(upstream, blocklist, cache);

    // Create hickory-dns server
    let mut server_future = ServerFuture::new(handler);

    // Register UDP listener if configured
    if let Some(ref udp_config) = protocols.udp {
        register_udp_listener(&mut server_future, udp_config).await?;
    }

    // Register TLS listener if configured
    if let Some(ref tls_config) = protocols.tls {
        register_tls_listener(&mut server_future, tls_config).await?;
    }

    // Ensure at least one protocol is configured
    if protocols.udp.is_none() && protocols.tls.is_none() {
        return Err(color_eyre::eyre::eyre!(
            "at least one protocol (UDP or TLS) must be configured"
        ));
    }

    let coordinator = ShutdownCoordinator::new();

    Ok(DnsServer {
        server_future,
        coordinator,
    })
}

/// Register UDP listener on the server
async fn register_udp_listener(
    server_future: &mut ServerFuture<DnsRequestHandler>,
    config: &UdpConfig,
) -> Result<()> {
    let addr = SocketAddr::new(config.host.into(), config.port);
    let socket = UdpSocket::bind(addr).await?;

    tracing::info!(%addr, "bound UDP socket for DNS queries");
    server_future.register_socket(socket);

    Ok(())
}

/// Register TLS listener on the server
async fn register_tls_listener(
    server_future: &mut ServerFuture<DnsRequestHandler>,
    config: &TlsConfig,
) -> Result<()> {
    let addr = SocketAddr::new(config.host.into(), config.port);

    // Load TLS configuration (async because it may fetch from S3)
    let tls_config = crate::tls::load_tls_config(config).await?;

    // Bind TCP listener for TLS
    let tcp_listener = TcpListener::bind(addr).await?;

    tracing::info!(%addr, "bound TLS listener for DNS over TLS queries");

    // Register listener with timeout (5 minutes for long-lived DoT connections)
    let timeout = std::time::Duration::from_secs(300);
    server_future.register_tls_listener(tcp_listener, timeout, tls_config)?;

    Ok(())
}
