use std::net::Ipv4Addr;

use serde::Deserialize;

#[derive(Clone, Debug, Deserialize)]
pub struct Configuration {
    pub server: ServerConfiguration,
    pub storage: StorageConfiguration,
}

#[derive(Clone, Debug, Deserialize)]
pub struct ServerConfiguration {
    pub host: Ipv4Addr,
    pub port: u16,
}

#[derive(Clone, Debug, Deserialize)]
pub struct StorageConfiguration {
    pub bucket: String,
    pub prefix: String,
}
