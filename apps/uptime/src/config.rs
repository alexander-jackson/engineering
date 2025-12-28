use std::net::SocketAddr;

use serde::Deserialize;

#[derive(Deserialize)]
pub struct Configuration {
    pub server: ServerConfiguration,
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
