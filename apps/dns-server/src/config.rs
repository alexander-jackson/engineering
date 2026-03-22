use std::net::Ipv4Addr;

use foundation_configuration::ExternalBytes;
use hickory_proto::xfer::Protocol;
use serde::Deserialize;

#[derive(Clone, Debug, Eq, PartialEq, Deserialize)]
pub struct Configuration {
    pub server: ServerConfig,
    pub upstream: UpstreamConfig,
    pub blocklist: BlocklistConfig,
    pub cache: CacheConfig,
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize)]
pub struct ServerConfig {
    pub protocols: ProtocolsConfig,
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize)]
pub struct ProtocolsConfig {
    pub tls: TlsConfig,
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize)]
pub struct TlsConfig {
    pub host: Ipv4Addr,
    pub port: u16,
    pub cert: ExternalBytes,
    pub key: ExternalBytes,
    pub refresh_interval_seconds: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize)]
pub struct UpstreamConfig {
    pub resolver: String,
    pub port: u16,
    pub protocol: Protocol,
    pub timeout_seconds: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize)]
pub struct BlocklistConfig {
    pub source: ExternalBytes,
    pub refresh_interval_seconds: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize)]
pub struct CacheConfig {
    pub max_entries: u64,
    pub default_ttl_seconds: u64,
}

#[cfg(test)]
mod tests {
    use std::net::Ipv4Addr;

    use color_eyre::eyre::Result;
    use foundation_configuration::ExternalBytes;
    use hickory_proto::xfer::Protocol;

    use crate::config::{
        BlocklistConfig, CacheConfig, Configuration, ProtocolsConfig, ServerConfig, TlsConfig,
        UpstreamConfig,
    };

    #[test]
    fn can_deserialize_sample_configuration() -> Result<()> {
        let yaml = include_str!("../resources/sample-config.yaml");

        let expected = Configuration {
            server: ServerConfig {
                protocols: ProtocolsConfig {
                    tls: TlsConfig {
                        host: Ipv4Addr::new(0, 0, 0, 0),
                        port: 853,
                        cert: ExternalBytes::S3 {
                            bucket: "configuration".into(),
                            key: "cert.pem".into(),
                        },
                        key: ExternalBytes::S3 {
                            bucket: "configuration".into(),
                            key: "key.pem".into(),
                        },
                        refresh_interval_seconds: 86400,
                    },
                },
            },
            upstream: UpstreamConfig {
                resolver: "all.dns.mullvad.net".to_string(),
                port: 443,
                protocol: Protocol::Https,
                timeout_seconds: 5,
            },
            blocklist: BlocklistConfig {
                source: ExternalBytes::Filesystem {
                    path: "/tmp/blocklist.txt".into(),
                },
                refresh_interval_seconds: 3600,
            },
            cache: CacheConfig {
                max_entries: 10000,
                default_ttl_seconds: 300,
            },
        };

        let actual: Configuration = serde_yaml::from_str(yaml)?;

        assert_eq!(actual, expected);

        Ok(())
    }
}
