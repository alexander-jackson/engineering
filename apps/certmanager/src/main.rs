use std::net::SocketAddrV4;

use color_eyre::eyre::Result;
use foundation_recurring_job::RecurringJob;
use foundation_shutdown::ShutdownCoordinator;
use foundation_templating::TemplateEngine;
use tokio::net::TcpListener;

mod acme;
mod configuration;
mod dns;
mod error;
mod persistence;
mod renewal;
mod server;
mod storage;
mod templates;
mod uid;
mod watcher;

use crate::acme::AcmeClient;
use crate::configuration::Configuration;
use crate::dns::DnsClient;
use crate::renewal::Renewer;
use crate::storage::CertificateStore;
use crate::watcher::Watcher;

#[tokio::main]
async fn main() -> Result<()> {
    let (config, pool) = foundation_init::run_with_bootstrap::<Configuration>().await?;
    let _ = rustls::crypto::ring::default_provider().install_default();

    let sdk_config = foundation_credentials::load().await?;
    let route53_client = aws_sdk_route53::Client::new(&sdk_config);
    let s3_client = aws_sdk_s3::Client::new(&sdk_config);

    let acme_client = AcmeClient::new(config.acme.environment, &config.acme.contact).await?;
    let dns_client = DnsClient::new(route53_client.clone());
    let cert_store = CertificateStore::new(s3_client.clone(), config.storage.clone());

    let renewer = Renewer::new(acme_client.clone(), dns_client.clone(), cert_store.clone());
    let watcher = Watcher::new(renewer.clone(), pool.clone());

    let template_engine = TemplateEngine::new()?;

    let addr = SocketAddrV4::new(config.server.host, config.server.port);
    let listener = TcpListener::bind(addr).await?;

    let server = crate::server::build(template_engine, renewer, pool, listener);

    ShutdownCoordinator::new()
        .with_task(server)
        .with_task(RecurringJob::new(watcher))
        .run()
        .await?;

    Ok(())
}
