use aws_config::BehaviorVersion;
use color_eyre::eyre::Result;
use reqwest::Client;
use tokio::net::TcpListener;

mod certificate_checker;
mod config;
mod persistence;
mod poller;
mod router;
mod templates;

use crate::certificate_checker::CertificateChecker;
use crate::config::Configuration;
use crate::poller::{AlertThreshold, CertificateAlertThreshold, Poller, PollerConfiguration};

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    let config = foundation_init::run::<Configuration>()?;
    let pool = foundation_database_bootstrap::run(&config.database).await?;

    let sdk_config = aws_config::load_defaults(BehaviorVersion::latest()).await;
    let sns_client = aws_sdk_sns::Client::new(&sdk_config);

    let configuration = PollerConfiguration::new(
        AlertThreshold::default(),
        CertificateAlertThreshold::default(),
        config.routing.sns_topic.clone(),
    );

    let http_client = Client::new();
    let poller = Poller::new(pool.clone(), http_client.clone(), sns_client, configuration);
    let cert_checker = CertificateChecker::new(pool.clone(), http_client);

    let router = crate::router::build(pool.clone())?;
    let listener = TcpListener::bind(config.server.addr).await?;

    tracing::info!(%config.server.addr, "listening for incoming requests");

    let _ = tokio::join!(
        poller.run(),
        cert_checker.run(),
        axum::serve(listener, router)
    );

    Ok(())
}
