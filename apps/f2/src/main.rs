use std::collections::HashMap;
use std::net::SocketAddrV4;
use std::sync::Arc;

use arc_swap::ArcSwap;
use color_eyre::eyre::Result;
use docker::client::{Client, DockerClient};
use foundation_shutdown::ShutdownCoordinator;
use rsa::RsaPrivateKey;
use service_registry::ServiceRegistry;
use tokio::net::TcpListener;
use tokio::sync::RwLock;

use crate::args::Args;
use crate::common::Container;
use crate::config::{Config, Service};
use crate::docker::api::create_and_start_container;
use crate::health::HealthMonitor;
use crate::ipc::channel;
use crate::load_balancer::LoadBalancer;
use crate::reconciler::Reconciler;

mod args;
mod common;
mod config;
mod crypto;
mod docker;
mod health;
mod ipc;
mod load_balancer;
mod reconciler;
mod service_registry;

fn setup() -> Result<()> {
    color_eyre::install()?;

    foundation_logging::install_default_registry();

    let _ = rustls::crypto::ring::default_provider().install_default();

    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    setup()?;

    let args = Args::parse()?;
    let config = Arc::new(ArcSwap::from_pointee(
        Config::from_location(&args.config_location).await?,
    ));

    let alb_config = &config.load().alb;

    let addr = alb_config.addr;
    let tls = alb_config.tls.clone();
    let mtls = alb_config.mtls.clone();

    let mut service_registry = ServiceRegistry::new();
    let private_key = config.load().get_private_key().await?;

    let docker_client = Client::default();
    let services = &config.load().services;

    start_services(
        &docker_client,
        services,
        &mut service_registry,
        private_key.as_ref(),
    )
    .await?;

    let service_registry = Arc::new(RwLock::new(service_registry));
    let (reconciliations, reconciliation_requests) = channel();
    let (health_checks, health_check_requests) = channel();
    let (health_responses, health_check_responses) = channel();

    let reconciler = Reconciler::new(
        Arc::clone(&service_registry),
        args.config_location.clone(),
        Arc::clone(&config),
        docker_client,
        reconciliation_requests,
        health_checks,
        health_check_responses,
    );

    let health_monitor = HealthMonitor::new(health_check_requests, health_responses);

    let mut listeners = HashMap::new();

    for (protocol, port) in alb_config.ports.iter() {
        let listener = TcpListener::bind(SocketAddrV4::new(addr, *port)).await?;
        listeners.insert(protocol.clone(), listener);
    }

    let load_balancer = LoadBalancer::new(
        service_registry,
        config,
        reconciliations,
        listeners,
        tls,
        mtls,
    );

    ShutdownCoordinator::new()
        .with_task(load_balancer)
        .with_task(reconciler)
        .with_task(health_monitor)
        .run()
        .await?;

    tracing::info!("shutting down gracefully, all components have completed their tasks");

    Ok(())
}

async fn start_services<C: DockerClient>(
    client: &C,
    services: &HashMap<String, Service>,
    service_registry: &mut ServiceRegistry,
    private_key: Option<&RsaPrivateKey>,
) -> Result<()> {
    for (name, service) in services {
        service_registry.define(name, service.clone());

        let tag = &service.tag;
        let container = Container::from(service);

        tracing::info!(%name, %tag, "starting service");

        for _ in 0..service.replicas.get() {
            let details = create_and_start_container(client, &container, tag, private_key).await?;
            service_registry.add_container(name, details);
        }
    }

    Ok(())
}
