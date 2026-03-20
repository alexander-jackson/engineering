use std::net::SocketAddrV4;

use aws_config::BehaviorVersion;
use axum::extract::State;
use axum::routing::put;
use axum::{Json, Router};
use color_eyre::eyre::Result;
use foundation_http_server::Server;
use serde::Deserialize;
use tokio::net::TcpListener;

mod acme;
mod configuration;
mod dns;
mod persistence;
mod storage;
mod uid;

use crate::acme::AcmeClient;
use crate::configuration::Configuration;
use crate::dns::DnsClient;
use crate::storage::CertificateStore;

#[derive(Clone)]
struct AppState {
    acme_client: AcmeClient,
    dns_client: DnsClient,
    cert_store: CertificateStore,
}

#[tokio::main]
async fn main() -> Result<()> {
    let config = foundation_init::run::<Configuration>()?;

    let sdk_config = aws_config::load_defaults(BehaviorVersion::latest()).await;
    let route53_client = aws_sdk_route53::Client::new(&sdk_config);
    let s3_client = aws_sdk_s3::Client::new(&sdk_config);

    let state = AppState {
        acme_client: AcmeClient::new().await?,
        dns_client: DnsClient::new(route53_client),
        cert_store: CertificateStore::new(s3_client, config.storage.clone()),
    };

    let addr = SocketAddrV4::new(config.server.host, config.server.port);
    let listener = TcpListener::bind(addr).await?;

    let router = Router::new()
        .route("/renew", put(renew_certificate))
        .with_state(state);

    let server = Server::new(router, listener);

    server.run().await?;

    Ok(())
}

#[derive(Debug, Deserialize)]
pub struct CertificateRenewalRequest {
    pub domain: String,
}

#[tracing::instrument(skip(acme_client, dns_client, cert_store))]
async fn renew_certificate(
    State(AppState {
        acme_client,
        dns_client,
        cert_store,
    }): State<AppState>,
    Json(request): Json<CertificateRenewalRequest>,
) -> String {
    let domain = request.domain;

    let (order, challenges) = acme_client.create_order(&domain).await.unwrap();

    for challenge in &challenges {
        dns_client
            .set_challenge_record(&challenge.identifier, &challenge.value)
            .await
            .unwrap();
    }

    let (private_key, cert_chain) = order.finalize().await.unwrap();

    cert_store
        .put(&domain, &private_key, &cert_chain)
        .await
        .unwrap();

    format!("Certificate for domain '{domain}' has been renewed.")
}
