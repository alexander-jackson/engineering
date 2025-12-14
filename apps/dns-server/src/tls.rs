use std::io::Cursor;
use std::sync::Arc;

use color_eyre::eyre::{Context, Result};
use rustls::server::{ResolvesServerCert, ServerConfig};

use crate::config::TlsConfig;

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
