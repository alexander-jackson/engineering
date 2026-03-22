use std::io::Cursor;
use std::sync::{Arc, RwLock};
use std::time::Duration;

use color_eyre::eyre::{Context, Result};
use foundation_shutdown::{CancellationToken, GracefulTask};
use rustls::server::{ClientHello, ResolvesServerCert, ServerConfig};
use rustls::sign::CertifiedKey;

use crate::config::TlsConfig;

#[derive(Clone, Debug)]
pub struct CertificateResolver {
    configuration: Arc<TlsConfig>,
    resolver: Arc<RwLock<Arc<dyn ResolvesServerCert>>>,
}

impl CertificateResolver {
    pub async fn new(configuration: TlsConfig) -> Result<Self> {
        let resolver = load_tls_config(&configuration).await?;

        Ok(Self {
            configuration: Arc::new(configuration),
            resolver: Arc::new(RwLock::new(resolver)),
        })
    }

    pub async fn reload(&self) -> Result<()> {
        let resolver = load_tls_config(&self.configuration).await?;
        let mut writer = self.resolver.write().unwrap();

        *writer = resolver;

        Ok(())
    }
}

impl GracefulTask for CertificateResolver {
    async fn run_until_shutdown(self, token: CancellationToken) -> Result<()> {
        let duration = Duration::from_secs(self.configuration.refresh_interval_seconds);
        let mut interval = tokio::time::interval(duration);

        loop {
            tokio::select! {
                _ = interval.tick() => {
                    if let Err(e) = self.reload().await {
                        tracing::error!(error = %e, "failed to reload TLS certificates");
                    }
                }
                _ = token.cancelled() => {
                    tracing::info!("received shutdown signal, stopping certificate resolver");
                    break;
                }
            }
        }

        Ok(())
    }
}

impl ResolvesServerCert for CertificateResolver {
    fn resolve(&self, client_hello: ClientHello<'_>) -> Option<Arc<CertifiedKey>> {
        self.resolver.read().unwrap().resolve(client_hello)
    }
}

/// Load TLS certificates and create cert resolver for hickory-server
#[tracing::instrument(skip(config))]
pub async fn load_tls_config(config: &TlsConfig) -> Result<Arc<dyn ResolvesServerCert>> {
    // Load certificate bytes from ExternalBytes (filesystem or S3)
    let cert_bytes = config
        .cert
        .resolve()
        .await
        .wrap_err("failed to load certificate")?;

    // Load private key bytes from ExternalBytes (filesystem or S3)
    let key_bytes = config
        .key
        .resolve()
        .await
        .wrap_err("failed to load private key")?;

    // Parse PEM files
    let mut cert_reader = Cursor::new(&cert_bytes[..]);
    let certs = rustls_pemfile::certs(&mut cert_reader)
        .collect::<Result<Vec<_>, _>>()
        .wrap_err("failed to parse certificate PEM")?;

    let mut key_reader = Cursor::new(&key_bytes[..]);
    let key = rustls_pemfile::private_key(&mut key_reader)
        .wrap_err("failed to parse private key PEM")?
        .ok_or_else(|| color_eyre::eyre::eyre!("no private key found"))?;

    // Create rustls server config with certificates
    let config = ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(certs, key)
        .wrap_err("failed to create TLS configuration")?;

    tracing::info!("loaded TLS certificates");

    // Extract the cert resolver from the server config
    Ok(config.cert_resolver)
}
