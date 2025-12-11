use std::net::Ipv4Addr;

use foundation_configuration::ExternalBytes;
use serde::Deserialize;

#[derive(Deserialize)]
pub struct ApplicationConfiguration {
    pub server: ServerConfig,
    pub upstream: UpstreamConfig,
    pub blocklist: BlocklistConfig,
    pub cache: CacheConfig,
}

#[derive(Deserialize)]
pub struct ServerConfig {
    pub host: Ipv4Addr,
    pub port: u16,
}

#[derive(Deserialize)]
pub struct UpstreamConfig {
    pub resolver: String,
    pub port: u16,
    pub timeout_seconds: u64,
}

#[derive(Clone, Deserialize)]
pub struct BlocklistConfig {
    pub source: ExternalBytes,
    pub refresh_interval_seconds: u64,
}

#[derive(Deserialize)]
pub struct CacheConfig {
    pub max_entries: u64,
    pub default_ttl_seconds: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_deserialization_filesystem() {
        let yaml = r#"
server:
  host: 0.0.0.0
  port: 5353

upstream:
  resolver: all.dns.mullvad.net
  port: 443
  timeout_seconds: 5

blocklist:
  source:
    location: filesystem
    path: /tmp/blocklist.txt
  refresh_interval_seconds: 3600

cache:
  max_entries: 10000
  default_ttl_seconds: 300
"#;

        let config: ApplicationConfiguration = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(config.server.port, 5353);
        assert_eq!(config.upstream.resolver, "all.dns.mullvad.net");
        assert_eq!(config.upstream.port, 443);
        assert_eq!(config.upstream.timeout_seconds, 5);
        assert_eq!(config.cache.max_entries, 10000);
        assert_eq!(config.cache.default_ttl_seconds, 300);
        assert_eq!(config.blocklist.refresh_interval_seconds, 3600);
    }

    #[test]
    fn test_config_deserialization_s3() {
        let yaml = r#"
server:
  host: 127.0.0.1
  port: 53

upstream:
  resolver: dns.example.com
  port: 443
  timeout_seconds: 10

blocklist:
  source:
    location: s3
    bucket: my-blocklists
    key: domains/blocked.txt
  refresh_interval_seconds: 7200

cache:
  max_entries: 5000
  default_ttl_seconds: 600
"#;

        let config: ApplicationConfiguration = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(config.server.port, 53);
        assert_eq!(config.upstream.timeout_seconds, 10);
        assert_eq!(config.blocklist.refresh_interval_seconds, 7200);
        assert_eq!(config.cache.max_entries, 5000);
    }
}
