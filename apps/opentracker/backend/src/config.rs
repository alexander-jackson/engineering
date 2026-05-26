use std::net::Ipv4Addr;

use serde::Deserialize;

#[derive(Deserialize)]
pub struct Configuration {
    pub server: ServerConfiguration,
    pub authorisation: AuthorisationConfiguration,
}

#[derive(Deserialize)]
pub struct ServerConfiguration {
    pub host: Ipv4Addr,
    pub port: u16,
}

#[derive(Deserialize)]
pub struct AuthorisationConfiguration {
    pub jwt_key: String,
}
