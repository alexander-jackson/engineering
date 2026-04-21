use std::net::SocketAddrV4;

use color_eyre::eyre::Result;
use foundation_shutdown::ShutdownCoordinator;
use foundation_templating::TemplateEngine;
use tokio::net::TcpListener;

mod config;
mod error;
mod persistence;
mod server;
mod templates;
mod uid;

use crate::config::Configuration;

#[tokio::main]
async fn main() -> Result<()> {
    let (config, pool) = foundation_init::run_with_bootstrap::<Configuration>().await?;

    let template_engine = TemplateEngine::new()?;

    let addr = SocketAddrV4::new(config.server.host, config.server.port);
    let listener = TcpListener::bind(addr).await?;
    let server = crate::server::build(template_engine, pool, listener);

    ShutdownCoordinator::new().with_task(server).run().await?;

    Ok(())
}

#[cfg(test)]
mod tests;
