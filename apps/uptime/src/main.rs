use color_eyre::eyre::Result;
use foundation_recurring_job::RecurringJob;
use foundation_shutdown::ShutdownCoordinator;
use reqwest::Client;
use tokio::net::TcpListener;

mod certificate_checker;
mod config;
mod persistence;
mod poller;
mod server;

use crate::certificate_checker::CertificateChecker;
use crate::config::Configuration;
use crate::poller::{AlertThreshold, CertificateAlertThreshold, Poller, PollerConfiguration};

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    let (config, pool) = foundation_init::run_with_bootstrap::<Configuration>().await?;

    let sdk_config = foundation_credentials::load().await?;
    let sns_client = aws_sdk_sns::Client::new(&sdk_config);

    let configuration = PollerConfiguration::new(
        AlertThreshold::default(),
        CertificateAlertThreshold::default(),
        config.routing.sns_topic.clone(),
    );

    let http_client = Client::new();
    let poller = Poller::new(pool.clone(), http_client.clone(), sns_client, configuration);
    let cert_checker = CertificateChecker::new(pool.clone(), http_client);

    let listener = TcpListener::bind(config.server.addr).await?;
    let server = crate::server::build(pool.clone(), listener)?;

    let poller_job = RecurringJob::new(poller);

    let cert_job = RecurringJob::new(cert_checker);

    ShutdownCoordinator::new()
        .with_task(poller_job)
        .with_task(cert_job)
        .with_task(server)
        .run()
        .await?;

    Ok(())
}
