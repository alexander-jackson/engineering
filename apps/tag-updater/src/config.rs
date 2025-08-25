use std::net::Ipv4Addr;

use foundation_configuration::{ExternalBytes, Secret};
use serde::Deserialize;

#[derive(Deserialize)]
pub struct Config {
    pub addr: Ipv4Addr,
    pub port: u16,
    pub passphrase: Secret<String>,
    pub private_key: ExternalBytes,
}
