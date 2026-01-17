use std::net::Ipv4Addr;

use serde::Deserialize;

#[derive(Deserialize)]
pub struct Configuration {
    pub server: ServerConfig,
}

#[derive(Deserialize)]
pub struct ServerConfig {
    pub host: Ipv4Addr,
    pub port: u16,
}
