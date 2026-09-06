use std::net::Ipv4Addr;

use foundation_configuration::Secret;
use hickory_net::xfer::Protocol;
use serde::Deserialize;

#[derive(Clone, Debug, Eq, PartialEq, Deserialize)]
pub struct Configuration {
    pub server: ServerConfig,
    pub upstream: UpstreamConfig,
    pub cache: CacheConfig,
    pub remote_blocklist: Option<RemoteBlocklistConfig>,
    pub classifier: Option<ClassifierConfig>,
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize)]
pub struct ServerConfig {
    pub dns: ListenerConfig,
    pub http: ListenerConfig,
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize)]
pub struct ListenerConfig {
    pub host: Ipv4Addr,
    pub port: u16,
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize)]
pub struct UpstreamConfig {
    pub resolver: String,
    pub port: u16,
    pub protocol: Protocol,
    pub timeout_seconds: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize)]
pub struct CacheConfig {
    pub max_entries: u64,
    pub default_ttl_seconds: u64,
    pub error_ttl_seconds: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize)]
pub struct RemoteBlocklistConfig {
    pub url: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize)]
pub struct ClassifierConfig {
    pub api_key: Secret<String>,
    pub model: String,
    /// How long a query waits for a new domain to be classified before it is allowed.
    pub timeout_seconds: u64,
    /// The most characters of page text sent to the model.
    pub max_content_chars: usize,
}

#[cfg(test)]
mod tests {
    use std::net::Ipv4Addr;

    use color_eyre::eyre::Result;
    use hickory_net::xfer::Protocol;

    use crate::config::{
        CacheConfig, ClassifierConfig, Configuration, ListenerConfig, RemoteBlocklistConfig,
        ServerConfig, UpstreamConfig,
    };

    #[test]
    fn can_deserialize_sample_configuration() -> Result<()> {
        let yaml = include_str!("../resources/sample-config.yaml");

        let expected = Configuration {
            server: ServerConfig {
                dns: ListenerConfig {
                    host: Ipv4Addr::new(0, 0, 0, 0),
                    port: 853,
                },
                http: ListenerConfig {
                    host: Ipv4Addr::new(0, 0, 0, 0),
                    port: 80,
                },
            },
            upstream: UpstreamConfig {
                resolver: "family.cloudflare-dns.com".to_string(),
                port: 443,
                protocol: Protocol::Https,
                timeout_seconds: 5,
            },
            cache: CacheConfig {
                max_entries: 10000,
                default_ttl_seconds: 300,
                error_ttl_seconds: 60,
            },
            remote_blocklist: Some(RemoteBlocklistConfig {
                url: "https://example.com/hagezi-multi.txt".to_string(),
            }),
            classifier: Some(ClassifierConfig {
                api_key: "sk-or-example".to_string().into(),
                model: "typesafe/jev-1.14".to_string(),
                timeout_seconds: 8,
                max_content_chars: 8000,
            }),
        };

        let actual: Configuration = serde_yaml::from_str(yaml)?;

        assert_eq!(actual, expected);

        Ok(())
    }
}
