use chrono::{DateTime, Utc};
use color_eyre::eyre::{eyre, Result};
use reqwest::Client;
use std::sync::Arc;
use tokio::net::TcpStream;
use tokio_rustls::rustls::client::danger::{
    HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier,
};
use tokio_rustls::rustls::pki_types::{CertificateDer, ServerName, UnixTime};
use tokio_rustls::rustls::{ClientConfig, DigitallySignedStruct, SignatureScheme};
use tokio_rustls::TlsConnector;
use x509_parser::certificate::X509Certificate;
use x509_parser::nom::Err as NomErr;
use x509_parser::prelude::FromDer;

/// A custom certificate verifier that captures the certificate chain for inspection
#[derive(Debug)]
struct CertificateCapture {
    captured_certs: Arc<std::sync::Mutex<Vec<CertificateDer<'static>>>>,
}

impl CertificateCapture {
    fn new() -> Self {
        Self {
            captured_certs: Arc::new(std::sync::Mutex::new(Vec::new())),
        }
    }

    fn get_captured_certs(&self) -> Vec<CertificateDer<'static>> {
        self.captured_certs.lock().unwrap().clone()
    }
}

impl ServerCertVerifier for CertificateCapture {
    fn verify_server_cert(
        &self,
        end_entity: &CertificateDer<'_>,
        intermediates: &[CertificateDer<'_>],
        _server_name: &ServerName<'_>,
        _ocsp_response: &[u8],
        _now: UnixTime,
    ) -> std::result::Result<ServerCertVerified, tokio_rustls::rustls::Error> {
        // Capture the certificate chain
        let mut certs = self.captured_certs.lock().unwrap();
        certs.clear();
        certs.push(end_entity.clone().into_owned());
        certs.extend(intermediates.iter().map(|c| c.clone().into_owned()));

        // Accept all certificates (we're just capturing them, not validating)
        Ok(ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        _message: &[u8],
        _cert: &CertificateDer<'_>,
        _dss: &DigitallySignedStruct,
    ) -> std::result::Result<HandshakeSignatureValid, tokio_rustls::rustls::Error> {
        Ok(HandshakeSignatureValid::assertion())
    }

    fn verify_tls13_signature(
        &self,
        _message: &[u8],
        _cert: &CertificateDer<'_>,
        _dss: &DigitallySignedStruct,
    ) -> std::result::Result<HandshakeSignatureValid, tokio_rustls::rustls::Error> {
        Ok(HandshakeSignatureValid::assertion())
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        vec![
            SignatureScheme::RSA_PKCS1_SHA256,
            SignatureScheme::RSA_PKCS1_SHA384,
            SignatureScheme::RSA_PKCS1_SHA512,
            SignatureScheme::ECDSA_NISTP256_SHA256,
            SignatureScheme::ECDSA_NISTP384_SHA384,
            SignatureScheme::ECDSA_NISTP521_SHA512,
            SignatureScheme::RSA_PSS_SHA256,
            SignatureScheme::RSA_PSS_SHA384,
            SignatureScheme::RSA_PSS_SHA512,
            SignatureScheme::ED25519,
        ]
    }
}

/// Extract the expiry date from a certificate
fn extract_certificate_expiry(cert_der: &[u8]) -> Result<DateTime<Utc>> {
    let (_, cert) = X509Certificate::from_der(cert_der)
        .map_err(|e: NomErr<_>| eyre!("Failed to parse certificate: {}", e))?;

    let not_after = cert.validity().not_after;
    let timestamp = not_after.timestamp();

    DateTime::from_timestamp(timestamp, 0)
        .ok_or_else(|| eyre!("Invalid timestamp in certificate"))
}

/// Check the certificate expiry for a given URI
pub async fn check_certificate_expiry(_client: &Client, uri: &str) -> Result<DateTime<Utc>> {
    // Ensure crypto provider is installed
    let _ = tokio_rustls::rustls::crypto::ring::default_provider().install_default();

    // Parse the URI to extract the host
    let url = uri.parse::<reqwest::Url>()?;
    let host = url
        .host_str()
        .ok_or_else(|| eyre!("No host in URI: {}", uri))?;
    let port = url.port().unwrap_or(443);

    // Create a custom TLS configuration with our certificate capturer
    let cert_capture = Arc::new(CertificateCapture::new());
    let mut tls_config = ClientConfig::builder()
        .dangerous()
        .with_custom_certificate_verifier(cert_capture.clone())
        .with_no_client_auth();

    // Enable ALPN for HTTP/1.1 and HTTP/2
    tls_config.alpn_protocols = vec![b"h2".to_vec(), b"http/1.1".to_vec()];

    // Connect to the server using TLS
    let addr = format!("{}:{}", host, port);
    let stream = TcpStream::connect(&addr).await?;

    let connector = TlsConnector::from(Arc::new(tls_config));
    let server_name = ServerName::try_from(host.to_string())?;

    // Perform the TLS handshake
    let _tls_stream = connector.connect(server_name, stream).await?;

    // Get the captured certificate
    let certs = cert_capture.get_captured_certs();
    let end_entity_cert = certs
        .first()
        .ok_or_else(|| eyre!("No certificate received from server"))?;

    // Extract and return the expiry date
    extract_certificate_expiry(end_entity_cert.as_ref())
}
