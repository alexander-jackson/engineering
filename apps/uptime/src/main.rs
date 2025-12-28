use aws_config::BehaviorVersion;
use color_eyre::eyre::Result;
use foundation_shutdown::ShutdownCoordinator;
use reqwest::Client;
use tokio::net::TcpListener;

mod certificate_checker;
mod config;
mod persistence;
mod poller;
mod server;
mod templates;

use crate::certificate_checker::CertificateChecker;
use crate::config::Configuration;
use crate::poller::{AlertThreshold, CertificateAlertThreshold, Poller, PollerConfiguration};

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    let (config, pool) = foundation_init::run_with_bootstrap::<Configuration>().await?;

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

    let server = crate::server::build(pool.clone())?;
    let listener = TcpListener::bind(config.server.addr).await?;

    let coordinator = ShutdownCoordinator::new();

    let poller_token = coordinator.token();
    let cert_token = coordinator.token();
    let server_token = coordinator.token();

    tokio::spawn(async move {
        coordinator.spawn().await.ok();
    });

    let _ = tokio::join!(
        poller.run(poller_token),
        cert_checker.run(cert_token),
        server.run_with_graceful_shutdown(listener, async move {
            server_token.cancelled().await;
        })
    );

    Ok(())
}
