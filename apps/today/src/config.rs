use std::net::Ipv4Addr;

use foundation_database_bootstrap::DatabaseConfiguration;
use serde::Deserialize;

#[derive(Deserialize)]
pub struct Configuration {
    pub server: ServerConfig,
    pub database: DatabaseConfiguration,
}

#[derive(Deserialize)]
pub struct ServerConfig {
    pub host: Ipv4Addr,
    pub port: u16,
}
