use std::net::Ipv4Addr;
use std::path::PathBuf;
use std::time::Duration;

use serde::Deserialize;

#[derive(Deserialize)]
pub struct Configuration {
    pub server: ServerConfiguration,
    #[serde(default)]
    pub query: QueryConfiguration,
    /// Where logs are flushed to, if they should be flushed at all.
    pub storage: Option<StorageConfiguration>,
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

/// Where completed hours of logs are written as Hive partitioned Parquet files.
#[derive(Clone, Debug, Deserialize)]
#[serde(tag = "location", rename_all = "lowercase")]
pub enum StorageConfiguration {
    /// A local directory, for running without S3.
    Filesystem { path: PathBuf },
    S3 {
        bucket: String,
        /// Everything is written beneath this prefix, which queries are also restricted to.
        prefix: String,
        #[serde(default = "default_region")]
        region: String,
        /// The CA bundle DuckDB uses for TLS, which has to be allowed explicitly.
        #[serde(default = "default_ca_certificate_file")]
        ca_certificate_file: String,
    },
}

fn default_region() -> String {
    "eu-west-1".to_owned()
}

fn default_ca_certificate_file() -> String {
    "/etc/ssl/certs/ca-certificates.crt".to_owned()
}

impl StorageConfiguration {
    pub fn is_s3(&self) -> bool {
        matches!(self, Self::S3 { .. })
    }
}
