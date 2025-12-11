use std::net::SocketAddr;

use color_eyre::eyre::Result;
use foundation_shutdown::ShutdownCoordinator;
use hickory_server::ServerFuture;
use tokio::net::UdpSocket;

use crate::blocklist::BlocklistManager;
use crate::cache::ResponseCache;
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

/// Build a DNS server
#[tracing::instrument(skip(upstream, blocklist, cache))]
pub async fn build(
    upstream: UpstreamResolver,
    blocklist: BlocklistManager,
    cache: ResponseCache,
    addr: SocketAddr,
) -> Result<DnsServer> {
    // Create the request handler
    let handler = DnsRequestHandler::new(upstream, blocklist, cache);

    // Create UDP socket for DNS
    let socket = UdpSocket::bind(addr).await?;
    tracing::info!(%addr, "bound UDP socket for DNS queries");

    // Create hickory-dns server
    let mut server_future = ServerFuture::new(handler);
    server_future.register_socket(socket);

    let coordinator = ShutdownCoordinator::new();

    Ok(DnsServer {
        server_future,
        coordinator,
    })
}
