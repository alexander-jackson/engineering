use std::net::Ipv4Addr;
use std::time::Duration;

use serde::Deserialize;

#[derive(Deserialize)]
pub struct Configuration {
    pub server: ServerConfiguration,
    #[serde(default)]
    pub query: QueryConfiguration,
}

#[derive(Deserialize)]
pub struct ServerConfiguration {
    pub host: Ipv4Addr,
    pub port: u16,
}

/// Limits on what a user-written query is allowed to consume.
#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(default)]
pub struct QueryConfiguration {
    /// How long a query may run before it is interrupted.
    pub timeout_millis: u64,
    /// The most rows a query returns, anything beyond this is cut off.
    pub max_rows: usize,
}

impl Default for QueryConfiguration {
    fn default() -> Self {
        Self {
            timeout_millis: 500,
            max_rows: 1000,
        }
    }
}

impl QueryConfiguration {
    pub fn timeout(&self) -> Duration {
        Duration::from_millis(self.timeout_millis)
    }
}
