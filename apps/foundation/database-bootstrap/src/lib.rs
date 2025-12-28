use std::path::Path;

use color_eyre::eyre::Result;
use foundation_configuration::Secret;
use serde::Deserialize;
use sqlx::migrate::Migrator;
use sqlx_bootstrap::{ApplicationConfig, BootstrapConfig, ConnectionConfig, RootConfig};

pub use sqlx::PgPool;

#[derive(Clone, Debug, Deserialize)]
pub struct DatabaseConfiguration {
    host: String,
    port: u16,
    root: DatabaseConnectionConfig,
    application: DatabaseConnectionConfig,
}

#[derive(Clone, Debug, Deserialize)]
pub struct DatabaseConnectionConfig {
    username: String,
    password: Secret<String>,
    database: String,
}

pub async fn run(config: &DatabaseConfiguration) -> Result<PgPool> {
    let root_username = &config.root.username;
    let root_password = &config.root.password;
    let root_database = &config.root.database;

    let app_username = &config.application.username;
    let app_password = &config.application.password;
    let app_database = &config.application.database;

    let host = &config.host;
    let port = config.port;

    let root_config = RootConfig::new(root_username, root_password, root_database);
    let app_config = ApplicationConfig::new(app_username, app_password, app_database);
    let conn_config = ConnectionConfig::new(host, port);

    let config = BootstrapConfig::new(root_config, app_config, conn_config);
    let pool = config.bootstrap().await?;

    let migrations = Path::new("./migrations");
    let migrator = Migrator::new(migrations).await?;

    migrator.run(&pool).await?;

    Ok(pool)
}
