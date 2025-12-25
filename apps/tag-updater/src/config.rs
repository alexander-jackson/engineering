use std::net::Ipv4Addr;
use std::path::PathBuf;

use foundation_configuration::{ExternalBytes, Secret};
use serde::Deserialize;

#[derive(Deserialize)]
pub struct Configuration {
    pub addr: Ipv4Addr,
    pub port: u16,
    pub passphrase: Secret<String>,
    pub private_key: ExternalBytes,
    pub repository: RepositoryConfiguration,
}

#[derive(Clone, Deserialize)]
pub struct RepositoryConfiguration {
    /// The URL of the Git repository, using SSH.
    pub url: String,
    /// The local path where the repository is cloned.
    pub local_path: PathBuf,
    /// The target path within the repository to operate on.
    pub target_path: PathBuf,
}
