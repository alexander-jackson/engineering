use std::net::SocketAddr;

use foundation_database_bootstrap::DatabaseConfiguration;
use serde::Deserialize;

#[derive(Deserialize)]
pub struct Configuration {
    pub server: ServerConfiguration,
    pub database: DatabaseConfiguration,
    pub routing: RoutingConfiguration,
}

#[derive(Deserialize)]
pub struct ServerConfiguration {
    pub addr: SocketAddr,
}

#[derive(Deserialize)]
pub struct RoutingConfiguration {
    pub sns_topic: String,
}
