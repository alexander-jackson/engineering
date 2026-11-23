use std::collections::HashMap;
use std::error::Error;
use std::io::Cursor;
use std::sync::Arc;

use arc_swap::ArcSwap;
use color_eyre::eyre::Result;
use foundation_shutdown::{CancellationToken, GracefulTask};
use http::{Request, Response};
use http_body_util::combinators::BoxBody;
use hyper::body::{Bytes, Incoming};
use hyper::service::{service_fn, Service};
use hyper_util::client::legacy::connect::HttpConnector;
use hyper_util::client::legacy::Client;
use hyper_util::rt::{TokioExecutor, TokioIo};
use hyper_util::server::conn::auto::Builder;
use mutual_tls::{ConnectionContext, Server, ServerConfiguration};
use rand::prelude::SmallRng;
use rustls::server::danger::ClientCertVerifier;
use rustls::server::{NoClientAuth, WebPkiClientVerifier};
use rustls::RootCertStore;
use tcp::TcpTlsProxy;
use tls::DynamicAuthenticationLevelResolver;
use tokio::net::TcpListener;
use tokio::sync::{Mutex, RwLock};
use tokio::task::JoinSet;
use tokio_rustls::TlsAcceptor;

use crate::config::{Config, MtlsConfig, Scheme, TlsConfig};
use crate::ipc::{channel, CertificateUpdateRequest, Receiver, ReconciliationRequest, Sender};
use crate::load_balancer::tls::CertificateResolver;
use crate::service_registry::ServiceRegistry;

mod proxy;
mod tcp;
mod tls;

#[derive(Debug)]
pub struct LoadBalancer {
    service_registry: Arc<RwLock<ServiceRegistry>>,
    client: Client<HttpConnector, Incoming>,
    rng: Arc<Mutex<SmallRng>>,
    config: Arc<ArcSwap<Config>>,
    reconciliations: Sender<ReconciliationRequest>,
    certificate_updates: Sender<CertificateUpdateRequest>,
    certificate_update_requests: Receiver<CertificateUpdateRequest>,
    listeners: HashMap<Scheme, TcpListener>,
    tls: Option<TlsConfig>,
    mtls: Option<MtlsConfig>,
}

impl LoadBalancer {
    pub fn new(
        service_registry: Arc<RwLock<ServiceRegistry>>,
        config: Arc<ArcSwap<Config>>,
        reconciliations: Sender<ReconciliationRequest>,
        listeners: HashMap<Scheme, TcpListener>,
        tls: Option<TlsConfig>,
        mtls: Option<MtlsConfig>,
    ) -> Self {
        let (certificate_updates, certificate_update_requests) = channel();
        let client = Client::builder(TokioExecutor::new()).build_http();
        let rng = Arc::new(Mutex::new(rand::make_rng()));

        Self {
            service_registry,
            client,
            rng,
            config,
            reconciliations,
            certificate_updates,
            certificate_update_requests,
            listeners,
            tls,
            mtls,
        }
    }

}

impl GracefulTask for LoadBalancer {
    async fn run_until_shutdown(self, shutdown: CancellationToken) -> Result<()> {
        let mut listeners = self.listeners;
        let tls = self.tls;
        let mtls = self.mtls;

        let reconciliation_path = Arc::from(self.config.load().alb.reconciliation.as_str());
        let reconciliations = self.reconciliations.clone();
        let certificate_updates = self.certificate_updates.clone();

        // Pre-clone Arcs needed after `service_factory` moves `self`
        let service_registry = Arc::clone(&self.service_registry);
        let rng = Arc::clone(&self.rng);
        let self_config = Arc::clone(&self.config);
        let certificate_update_requests = self.certificate_update_requests;

        let service_factory = move |_| {
            let service_registry = Arc::clone(&self.service_registry);
            let rng = Arc::clone(&self.rng);
            let client = self.client.clone();
            let reconciliation_path = Arc::clone(&reconciliation_path);
            let reconciliations = reconciliations.clone();
            let certificate_updates = certificate_updates.clone();

            service_fn(move |req| {
                let service_registry = Arc::clone(&service_registry);
                let rng = Arc::clone(&rng);
                let client = client.clone();
                let reconciliation_path = Arc::clone(&reconciliation_path);
                let reconciliations = reconciliations.clone();
                let certificate_updates = certificate_updates.clone();

                async move {
                    proxy::handle_request(
                        service_registry,
                        rng,
                        client,
                        reconciliation_path,
                        reconciliations,
                        certificate_updates,
                        req,
                    )
                    .await
                }
            })
        };

        let mut tasks = JoinSet::new();

        if let Some(listener) = listeners.remove(&Scheme::Http) {
            let server = HttpServer::new(service_factory.clone());

            tracing::info!("starting http server on {}", listener.local_addr()?);

            tasks.spawn(server.run(listener));
        }

        // One resolver serves every TLS listener so that a certificate update reaches all of them
        let wants_tls =
            listeners.contains_key(&Scheme::Https) || listeners.contains_key(&Scheme::Tls);
        let certificate_resolver = match tls.as_ref() {
            Some(tls) if wants_tls => {
                let config = Arc::new(tls.domains.clone());
                let resolver = CertificateResolver::new(config, certificate_update_requests);

                Some(Arc::new(resolver.await?))
            }
            _ => None,
        };

        if let Some(listener) = listeners.remove(&Scheme::Https) {
            if let Some(certificate_resolver) = certificate_resolver.as_ref() {
                let client_cert_verifier: Arc<dyn ClientCertVerifier> = match &mtls {
                    Some(config) => {
                        let bytes = config.anchor.resolve().await?;
                        let mut cursor = Cursor::new(bytes);

                        let mut store = RootCertStore::empty();
                        let certs = rustls_pemfile::certs(&mut cursor).filter_map(Result::ok);
                        let (added, ignored) = store.add_parsable_certificates(certs);

                        tracing::info!(%added, %ignored, "set up the trust store");

                        WebPkiClientVerifier::builder(Arc::new(store)).build()?
                    }
                    None => Arc::new(NoClientAuth),
                };

                let authentication_level_resolver =
                    DynamicAuthenticationLevelResolver::new(Arc::clone(&self_config));

                let server_configuration = ServerConfiguration::default();
                let server = Server::new(
                    authentication_level_resolver,
                    client_cert_verifier,
                    certificate_resolver.clone(),
                    service_factory,
                    server_configuration,
                );

                tracing::info!("starting https server on {}", listener.local_addr()?);

                tasks.spawn(server.run(listener));
            }
        }

        if let Some(listener) = listeners.remove(&Scheme::Tls) {
            if let Some(certificate_resolver) = certificate_resolver.as_ref() {
                let server_config = rustls::ServerConfig::builder()
                    .with_no_client_auth()
                    .with_cert_resolver(certificate_resolver.clone());

                let acceptor = TlsAcceptor::from(Arc::new(server_config));
                let proxy =
                    TcpTlsProxy::new(Arc::clone(&service_registry), Arc::clone(&rng), acceptor);

                tracing::info!("starting TCP TLS proxy on {}", listener.local_addr()?);

                tasks.spawn(proxy.run(listener));
            }
        }

        tracing::info!("waiting for all servers to complete");

        tokio::select! {
            _ = shutdown.cancelled() => {
                // Dropping the set stops the accept loops, connections already accepted carry on
                tracing::info!("shutting down the load balancer, no longer accepting connections");
            }
            _ = tasks.join_all() => {
                tracing::info!("all servers have completed");
            }
        }

        Ok(())
    }
}

pub struct HttpServer<F> {
    service_factory: Arc<F>,
}

impl<F, S> HttpServer<F>
where
    F: Fn(ConnectionContext) -> S + Send + Sync + 'static,
    S: Service<Request<Incoming>, Response = Response<BoxBody<Bytes, hyper::Error>>>
        + Send
        + 'static,
    S::Future: 'static,
    <S as Service<Request<Incoming>>>::Future: Send,
    <S as Service<Request<Incoming>>>::Error: Into<Box<dyn Error + Send + Sync>>,
{
    pub fn new(service_factory: F) -> Self {
        Self {
            service_factory: Arc::new(service_factory),
        }
    }

    async fn run(self, mut listener: TcpListener) {
        loop {
            if let Err(e) = self.try_handle_connection(&mut listener).await {
                tracing::warn!(%e, "failed to handle connection");
            } else {
                tracing::trace!("handled a connection from a client");
            }
        }
    }

    pub async fn try_handle_connection(
        &self,
        listener: &mut TcpListener,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        let (stream, _) = listener.accept().await?;
        let io = TokioIo::new(stream);

        let service = (self.service_factory)(ConnectionContext { common_name: None });

        tokio::spawn(async move {
            if let Err(e) = Builder::new(TokioExecutor::new())
                .serve_connection(io, service)
                .await
            {
                tracing::warn!(%e, "error handling connection");
            }
        });

        Ok(())
    }
}

#[cfg(test)]
mod tests;
