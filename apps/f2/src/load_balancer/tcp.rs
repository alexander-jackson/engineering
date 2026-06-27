use std::net::SocketAddrV4;
use std::sync::Arc;

use color_eyre::eyre::{eyre, Result};
use rand::prelude::SmallRng;
use rand::Rng;
use tokio::io::copy_bidirectional;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{Mutex, RwLock};
use tokio_rustls::TlsAcceptor;

use crate::service_registry::ServiceRegistry;

pub struct TcpTlsProxy {
    service_registry: Arc<RwLock<ServiceRegistry>>,
    rng: Arc<Mutex<SmallRng>>,
    acceptor: TlsAcceptor,
}

impl TcpTlsProxy {
    pub fn new(
        service_registry: Arc<RwLock<ServiceRegistry>>,
        rng: Arc<Mutex<SmallRng>>,
        acceptor: TlsAcceptor,
    ) -> Self {
        Self {
            service_registry,
            rng,
            acceptor,
        }
    }

    pub async fn run(self, mut listener: TcpListener) {
        loop {
            if let Err(e) = self.try_handle_connection(&mut listener).await {
                tracing::warn!(%e, "failed to handle TCP TLS connection");
            }
        }
    }

    async fn try_handle_connection(&self, listener: &mut TcpListener) -> Result<()> {
        let (stream, peer_addr) = listener.accept().await?;

        let acceptor = self.acceptor.clone();
        let service_registry = Arc::clone(&self.service_registry);
        let rng = Arc::clone(&self.rng);

        tokio::spawn(async move {
            if let Err(e) = handle_connection(acceptor, service_registry, rng, stream).await {
                tracing::warn!(%peer_addr, %e, "error handling TCP TLS connection");
            }
        });

        Ok(())
    }
}

async fn handle_connection(
    acceptor: TlsAcceptor,
    service_registry: Arc<RwLock<ServiceRegistry>>,
    rng: Arc<Mutex<SmallRng>>,
    stream: TcpStream,
) -> Result<()> {
    let mut tls_stream = acceptor.accept(stream).await?;

    let sni = tls_stream
        .get_ref()
        .1
        .server_name()
        .ok_or_else(|| eyre!("no SNI hostname in TLS handshake"))?
        .to_owned();

    let read_lock = service_registry.read().await;

    let Some((downstreams, port)) = read_lock.find_downstreams(&sni, "") else {
        return Err(eyre!("no downstream found for SNI hostname {sni}"));
    };

    let downstream_addr = {
        let mut rng = rng.lock().await;
        let idx = rng.next_u32() as usize % downstreams.len();
        downstreams
            .get_index(idx)
            .ok_or_else(|| eyre!("no downstream available for {sni}"))?
            .addr
    };

    drop(read_lock);

    let addr = SocketAddrV4::new(downstream_addr, port);
    let mut backend = TcpStream::connect(addr).await?;

    copy_bidirectional(&mut tls_stream, &mut backend).await?;

    Ok(())
}
