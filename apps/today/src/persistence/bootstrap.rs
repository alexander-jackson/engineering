use color_eyre::eyre::Result;
use sqlx::PgPool;
use sqlx_bootstrap::{ApplicationConfig, BootstrapConfig, ConnectionConfig, RootConfig};

use crate::config::DatabaseConfig;

pub async fn run(config: &DatabaseConfig) -> Result<PgPool> {
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

    sqlx::migrate!().run(&pool).await?;

    Ok(pool)
}
