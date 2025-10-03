use std::net::SocketAddrV4;

use color_eyre::eyre::Result;
use foundation_init::Configuration;
use tokio::net::TcpListener;

use crate::server::IndexCache;

mod config;
mod error;
mod persistence;
mod server;
mod templates;
mod uid;

use crate::config::ApplicationConfiguration;
use crate::templates::TemplateEngine;

#[tokio::main]
async fn main() -> Result<()> {
    let config: Configuration<ApplicationConfiguration> = foundation_init::run()?;

    let template_engine = TemplateEngine::new()?;
    let pool = crate::persistence::bootstrap::run(&config.database).await?;
    let index_cache = IndexCache::new(32);

    let addr = SocketAddrV4::new(config.server.host, config.server.port);
    let server = crate::server::build(template_engine, pool, index_cache);
    let listener = TcpListener::bind(addr).await?;

    server.run(listener).await?;

    Ok(())
}

#[cfg(test)]
mod tests;
