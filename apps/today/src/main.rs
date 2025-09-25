use std::net::SocketAddrV4;

use color_eyre::eyre::Result;
use foundation_args::Args;
use foundation_configuration::ConfigurationReader;
use tokio::net::TcpListener;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

use crate::server::IndexCache;

mod config;
mod error;
mod persistence;
mod server;
mod templates;
mod uid;

use crate::config::Config;
use crate::templates::TemplateEngine;

fn setup(config: &Config) -> Result<()> {
    dotenvy::dotenv().ok();
    color_eyre::install()?;

    let registry = foundation_logging::get_default_registry();

    match &config.telemetry {
        Some(telemetry) if telemetry.enabled => {
            let layer = foundation_telemetry::get_trace_layer("today", &telemetry.endpoint)?;
            registry.with(layer).init();
        }
        _ => {
            registry.init();
        }
    }

    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::from_env()?;
    let config = Config::from_yaml(&args.config)?;

    setup(&config)?;

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
