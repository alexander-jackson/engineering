use std::sync::Arc;
use std::time::Duration;

use color_eyre::eyre::{Context, Result, eyre};
use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::pki_types::{CertificateDer, ServerName, UnixTime};
use rustls::{DigitallySignedStruct, Error as TlsError, SignatureScheme};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio_rustls::TlsConnector;

mod docker;

fn main() -> Result<()> {
    color_eyre::install()?;

    setup_dependencies().wrap_err("Failed to set up dependencies")?;

    // check that volumes work correctly
    check_volumes_work().wrap_err("Volumes did not work correctly")?;

    // check that args are passed through and override the image CMD correctly
    // (must run before check_rolls_work, whose reconciler prunes all unused images)
    check_args_work().wrap_err("Args did not work correctly")?;

    // check that the TCP TLS proxy correctly forwards bytes after TLS termination
    // (must run before check_rolls_work, whose reconciler prunes all unused images)
    check_tls_tcp_proxy_works().wrap_err("TCP TLS proxy did not work correctly")?;

    // check that rolls work correctly
    check_rolls_work().wrap_err("Rolls did not work correctly")?;

    Ok(())
}

/// Builds the necessary Docker images and creates the network for the tests to run.
fn setup_dependencies() -> Result<()> {
    // build the main image
    crate::docker::build("development/Dockerfile.debug", ".", "f2", "debug")?;

    // build the supporting images
    crate::docker::build(
        "development/servers/echo/Dockerfile.single",
        "development/servers/echo",
        "echo",
        "single",
    )?;

    crate::docker::build(
        "development/servers/echo/Dockerfile.double",
        "development/servers/echo",
        "echo",
        "double",
    )?;

    crate::docker::build(
        "development/servers/volumes/Dockerfile",
        "development/servers/volumes",
        "volumes",
        "latest",
    )?;

    crate::docker::build(
        "development/servers/args/Dockerfile",
        "development/servers/args",
        "args",
        "latest",
    )?;

    crate::docker::build(
        "development/servers/tcp-echo/Dockerfile",
        "development/servers/tcp-echo",
        "tcp-echo",
        "latest",
    )?;

    // create the internal network
    crate::docker::create_network_if_not_exists("internal")?;

    Ok(())
}

fn check_volumes_work() -> Result<()> {
    // run the main container
    let volumes = vec![
        ("./development", "/development"),
        ("/var/run/docker.sock", "/var/run/docker.sock"),
    ];

    crate::docker::run(
        "f2",
        "debug",
        &volumes,
        "/development/volumes-config.yaml",
        &[("3000", "3000")],
    )?;

    // give the container a moment to start up
    std::thread::sleep(Duration::from_secs(1));

    // check we can get a response from it
    let expected = std::fs::read_to_string("development/volumes-configuration.json")?;
    assert_response_equals("http://localhost:3000", &expected)?;

    // remove the running containers
    crate::docker::remove_running_containers()?;

    Ok(())
}

fn check_rolls_work() -> Result<()> {
    // run the main container
    let volumes = vec![
        ("./development", "/development"),
        ("/var/run/docker.sock", "/var/run/docker.sock"),
    ];

    // start the next test
    crate::docker::run(
        "f2",
        "debug",
        &volumes,
        "/development/echo-single-config.yaml",
        &[("3000", "3000")],
    )?;

    // give the container a moment to start up
    std::thread::sleep(Duration::from_secs(1));

    // check we can get a response from it
    assert_response_equals("http://localhost:3000/foobar", "Echo foobar")?;

    // roll to a new version
    swap(
        "./development/echo-single-config.yaml",
        "./development/echo-double-config.yaml",
    )?;

    // force a reconciliation
    let mut response = ureq::put("http://localhost:3000/reconcile").send_empty()?;

    if !response.status().is_success() {
        return Err(eyre!(
            "Failed to trigger reconciliation: {}",
            response.body_mut().read_to_string()?
        ));
    }

    // give the container a moment to reconcile
    std::thread::sleep(Duration::from_secs(1));

    // check we can get a response from it
    assert_response_equals("http://localhost:3000/foobar", "Echo echo foobar")?;

    crate::docker::remove_running_containers()?;

    swap(
        "./development/echo-single-config.yaml",
        "./development/echo-double-config.yaml",
    )?;

    Ok(())
}

fn check_args_work() -> Result<()> {
    let volumes = vec![
        ("./development", "/development"),
        ("/var/run/docker.sock", "/var/run/docker.sock"),
    ];

    // without args: Dockerfile CMD is preserved, server responds with "default"
    crate::docker::run(
        "f2",
        "debug",
        &volumes,
        "/development/args-default-config.yaml",
        &[("3000", "3000")],
    )?;
    std::thread::sleep(Duration::from_secs(1));
    assert_response_equals("http://localhost:3000", "default")?;
    crate::docker::remove_running_containers()?;

    // with args: f2 overrides CMD, server responds with "overridden"
    crate::docker::run(
        "f2",
        "debug",
        &volumes,
        "/development/args-override-config.yaml",
        &[("3000", "3000")],
    )?;
    std::thread::sleep(Duration::from_secs(1));
    assert_response_equals("http://localhost:3000", "overridden")?;
    crate::docker::remove_running_containers()?;

    Ok(())
}

/// A TLS certificate verifier that accepts any certificate, for use in integration tests only.
#[derive(Debug)]
struct AcceptAnyCert;

impl ServerCertVerifier for AcceptAnyCert {
    fn verify_server_cert(
        &self,
        _end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _server_name: &ServerName<'_>,
        _ocsp_response: &[u8],
        _now: UnixTime,
    ) -> std::result::Result<ServerCertVerified, TlsError> {
        Ok(ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dsa: &DigitallySignedStruct,
    ) -> std::result::Result<HandshakeSignatureValid, TlsError> {
        rustls::crypto::verify_tls12_signature(
            message,
            cert,
            dsa,
            &rustls::crypto::ring::default_provider().signature_verification_algorithms,
        )
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dsa: &DigitallySignedStruct,
    ) -> std::result::Result<HandshakeSignatureValid, TlsError> {
        rustls::crypto::verify_tls13_signature(
            message,
            cert,
            dsa,
            &rustls::crypto::ring::default_provider().signature_verification_algorithms,
        )
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        rustls::crypto::ring::default_provider()
            .signature_verification_algorithms
            .supported_schemes()
    }
}

fn check_tls_tcp_proxy_works() -> Result<()> {
    let volumes = vec![
        ("./development", "/development"),
        ("./resources", "/resources"),
        ("/var/run/docker.sock", "/var/run/docker.sock"),
    ];

    crate::docker::run(
        "f2",
        "debug",
        &volumes,
        "/development/tcp-tls-config.yaml",
        &[("4001", "4001")],
    )?;

    std::thread::sleep(Duration::from_secs(1));

    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?
        .block_on(async {
            let client_config = rustls::ClientConfig::builder()
                .dangerous()
                .with_custom_certificate_verifier(Arc::new(AcceptAnyCert))
                .with_no_client_auth();

            let connector = TlsConnector::from(Arc::new(client_config));
            let stream = TcpStream::connect("127.0.0.1:4001").await?;
            let domain = ServerName::try_from("old.example.com")?;
            let mut tls = connector.connect(domain, stream).await?;

            tls.write_all(b"hello").await?;
            tls.flush().await?;

            let mut buf = [0u8; 5];
            tls.read_exact(&mut buf).await?;

            assert_eq!(&buf, b"hello", "TCP TLS proxy did not echo bytes correctly");

            color_eyre::eyre::Ok(())
        })?;

    println!("✅ TCP TLS proxy forwarded bytes correctly");

    crate::docker::remove_running_containers()?;

    Ok(())
}

fn assert_response_equals(uri: &str, content: &str) -> Result<()> {
    let mut response = ureq::get(uri).call()?;
    let response_text = response.body_mut().read_to_string()?;

    if response_text != content {
        return Err(eyre!(
            "Received unexpected response from container: {response_text}, expected: {content} when calling {uri}"
        ));
    }

    println!("✅ Successfully received correct response from {uri}");

    Ok(())
}

fn swap(path1: &str, path2: &str) -> Result<()> {
    std::fs::rename(path1, format!("{}.tmp", path1))?;
    std::fs::rename(path2, path1)?;
    std::fs::rename(format!("{}.tmp", path1), path2)?;

    println!("✅ Successfully swapped {path1} and {path2}");

    Ok(())
}
