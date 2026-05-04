use std::net::Ipv4Addr;

use foundation_configuration::Secret;
use serde::Deserialize;

use crate::acme::Environment;

#[derive(Clone, Debug, Deserialize)]
pub struct Configuration {
    pub server: ServerConfiguration,
    pub storage: StorageConfiguration,
    pub acme: AcmeConfiguration,
    pub notify: NotifyConfiguration,
}

#[derive(Clone, Debug, Deserialize)]
pub struct NotifyConfiguration {
    pub url: String,
    pub path: String,
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

#[derive(Clone, Debug, Deserialize)]
pub struct AcmeConfiguration {
    pub contact: Secret<String>,
    pub environment: Environment,
}
