use std::net::Ipv4Addr;

use foundation_configuration::Secret;
use serde::Deserialize;

#[derive(Deserialize)]
pub struct ApplicationConfiguration {
    pub server: ServerConfig,
    pub database: DatabaseConfig,
}

#[derive(Deserialize)]
pub struct ServerConfig {
    pub host: Ipv4Addr,
    pub port: u16,
}

#[derive(Clone, Debug, Deserialize)]
pub struct DatabaseConfig {
    pub host: String,
    pub port: u16,
    pub root: DatabaseConnectionConfig,
    pub application: DatabaseConnectionConfig,
}

#[derive(Clone, Debug, Deserialize)]
pub struct DatabaseConnectionConfig {
    pub username: String,
    pub password: Secret<String>,
    pub database: String,
}
