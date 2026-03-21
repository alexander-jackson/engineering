use std::net::SocketAddrV4;

use aws_config::BehaviorVersion;
use axum::extract::State;
use axum::routing::put;
use axum::{Json, Router};
use color_eyre::eyre::Result;
use foundation_http_server::Server;
use foundation_shutdown::ShutdownCoordinator;
use serde::Deserialize;
use sqlx::PgPool;
use sqlx::types::chrono::Utc;
use tokio::net::TcpListener;

mod acme;
mod configuration;
mod dns;
mod persistence;
mod renewal;
mod storage;
mod uid;
mod watcher;

use crate::acme::AcmeClient;
use crate::configuration::Configuration;
use crate::dns::DnsClient;
use crate::renewal::Renewer;
use crate::storage::CertificateStore;
use crate::watcher::Watcher;

#[derive(Clone)]
struct AppState {
    renewer: Renewer,
    pool: PgPool,
}

#[tokio::main]
async fn main() -> Result<()> {
    let (config, pool) = foundation_init::run_with_bootstrap::<Configuration>().await?;
    let _ = rustls::crypto::ring::default_provider().install_default();

    let sdk_config = aws_config::load_defaults(BehaviorVersion::latest()).await;
    let route53_client = aws_sdk_route53::Client::new(&sdk_config);
    let s3_client = aws_sdk_s3::Client::new(&sdk_config);

    let acme_client = AcmeClient::new().await?;
    let dns_client = DnsClient::new(route53_client.clone());
    let cert_store = CertificateStore::new(s3_client.clone(), config.storage.clone());

    let renewer = Renewer::new(acme_client.clone(), dns_client.clone(), cert_store.clone());

    let state = AppState {
        renewer: renewer.clone(),
        pool: pool.clone(),
    };

    let watcher = Watcher::new(renewer.clone(), pool.clone());

    let addr = SocketAddrV4::new(config.server.host, config.server.port);
    let listener = TcpListener::bind(addr).await?;

    let router = Router::new()
        .route("/register", put(register_domain))
        .with_state(state);

    let server = Server::new(router, listener);

    ShutdownCoordinator::new()
        .with_task(server)
        .with_task(watcher)
        .run()
        .await?;

    Ok(())
}

#[derive(Debug, Deserialize)]
pub struct DomainRegistrationRequest {
    pub domain: String,
}

#[tracing::instrument(skip(renewer, pool))]
async fn register_domain(
    State(AppState { renewer, pool }): State<AppState>,
    Json(request): Json<DomainRegistrationRequest>,
) -> String {
    let domain = request.domain;

    let mut tx = pool.begin().await.expect("Failed to begin transaction");

    let domain_uid = crate::persistence::insert_domain(&mut tx, &domain)
        .await
        .expect("Failed to insert domain");

    let expires_at = renewer.renew(&domain).await.unwrap();

    let certificate_uid =
        crate::persistence::insert_certificate(&mut tx, domain_uid, Utc::now(), expires_at)
            .await
            .expect("Failed to insert certificate");

    tx.commit().await.expect("Failed to commit transaction");

    tracing::info!(domain, %domain_uid, %certificate_uid, "Domain registered and certificate issued");

    expires_at.to_string()
}
