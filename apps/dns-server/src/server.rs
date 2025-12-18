use std::net::SocketAddr;
use std::time::Duration;

use color_eyre::eyre::Result;
use foundation_shutdown::ShutdownCoordinator;
use hickory_server::ServerFuture;
use tokio::net::{TcpListener, UdpSocket};

use crate::blocklist::BlocklistManager;
use crate::cache::ResponseCache;
use crate::config::{ProtocolsConfig, TlsConfig, UdpConfig};
use crate::handler::DnsRequestHandler;
use crate::upstream::UpstreamResolver;

pub struct DnsServer {
    server_future: ServerFuture<DnsRequestHandler>,
    coordinator: ShutdownCoordinator,
}

impl DnsServer {
    #[tracing::instrument(skip(upstream, blocklist, cache))]
    pub async fn new(
        upstream: UpstreamResolver,
        blocklist: BlocklistManager,
        cache: ResponseCache,
        protocols: &ProtocolsConfig,
    ) -> Result<Self> {
        let handler = DnsRequestHandler::new(upstream, blocklist, cache);
        let mut server_future = ServerFuture::new(handler);

        if protocols.udp.is_none() && protocols.tls.is_none() {
            return Err(color_eyre::eyre::eyre!(
                "at least one protocol (UDP or TLS) must be configured"
            ));
        }

        if let Some(ref udp_config) = protocols.udp {
            register_udp_listener(&mut server_future, udp_config).await?;
        }

        if let Some(ref tls_config) = protocols.tls {
            register_tls_listener(&mut server_future, tls_config).await?;
        }

        let coordinator = ShutdownCoordinator::new();

        Ok(Self {
            server_future,
            coordinator,
        })
    }

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

async fn register_tls_listener(
    server_future: &mut ServerFuture<DnsRequestHandler>,
    config: &TlsConfig,
) -> Result<()> {
    let addr = SocketAddr::new(config.host.into(), config.port);

    let config = crate::tls::load_tls_config(config).await?;
    let listener = TcpListener::bind(addr).await?;

    tracing::info!(%addr, "bound TLS listener for DNS over TLS queries");

    let timeout = Duration::from_secs(300);
    server_future.register_tls_listener(listener, timeout, config)?;

    Ok(())
}
