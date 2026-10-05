use std::net::SocketAddrV4;

use color_eyre::eyre::Result;
use foundation_recurring_job::RecurringJob;
use foundation_shutdown::ShutdownCoordinator;
use foundation_templating::TemplateEngine;
use tokio::net::TcpListener;

mod config;
mod conversation;
mod error;
mod openrouter;
mod persistence;
mod prompt;
mod server;
mod telegram;
mod templates;
mod uid;
mod weekly_job;

use crate::config::Configuration;
use crate::openrouter::OpenRouterClient;
use crate::telegram::TelegramClient;
use crate::weekly_job::WeeklyRecipes;

#[tokio::main]
async fn main() -> Result<()> {
    let (config, pool) = foundation_init::run_with_bootstrap::<Configuration>().await?;

    let template_engine = TemplateEngine::new()?;
    let openrouter = OpenRouterClient::new(&config.openrouter)?;
    let telegram = TelegramClient::new(&config.telegram)?;

    let job = WeeklyRecipes::new(
        pool.clone(),
        openrouter.clone(),
        telegram,
        &config.base_url,
        config.schedule,
    );

    let addr = SocketAddrV4::new(config.server.host, config.server.port);
    let listener = TcpListener::bind(addr).await?;
    let server = crate::server::build(template_engine, pool, openrouter, listener);

    ShutdownCoordinator::new()
        .with_task(RecurringJob::new(job))
        .with_task(server)
        .run()
        .await?;

    Ok(())
}

#[cfg(test)]
mod tests;
