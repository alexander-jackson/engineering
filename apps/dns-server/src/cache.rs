use std::sync::Arc;
use std::time::{Duration, Instant};

use hickory_proto::op::Message;
use moka::future::Cache;
use moka::Expiry;

use crate::config::CacheConfig;

/// Cached DNS response with TTL metadata
#[derive(Clone)]
struct CachedResponse {
    message: Message,
    ttl: Duration,
}

/// Custom expiry policy that respects per-entry TTL
struct DnsExpiry;

impl Expiry<String, Arc<CachedResponse>> for DnsExpiry {
    fn expire_after_create(
        &self,
        _key: &String,
        value: &Arc<CachedResponse>,
        _current_time: Instant,
    ) -> Option<Duration> {
        Some(value.ttl)
    }
}

/// DNS response cache with TTL-based eviction
#[derive(Clone)]
pub struct ResponseCache {
    cache: Cache<String, Arc<CachedResponse>>,
    default_ttl: Duration,
}

impl ResponseCache {
    /// Create a new response cache
    pub fn new(config: &CacheConfig) -> Self {
        let cache = Cache::builder()
            .max_capacity(config.max_entries)
            .expire_after(DnsExpiry)
            .build();

        let default_ttl = Duration::from_secs(config.default_ttl_seconds);

        tracing::info!(
            max_entries = config.max_entries,
            default_ttl_seconds = config.default_ttl_seconds,
            "initialized DNS response cache"
        );

        Self { cache, default_ttl }
    }

    /// Get a cached response
    #[tracing::instrument(skip(self))]
    pub async fn get(&self, key: &str) -> Option<Message> {
        match self.cache.get(key).await {
            Some(cached) => {
                tracing::debug!(key = %key, "cache hit");
                Some(cached.message.clone())
            }
            None => {
                tracing::debug!(key = %key, "cache miss");
                None
            }
        }
    }

    /// Insert a response into the cache with optional TTL
    #[tracing::instrument(skip(self, message))]
    pub async fn insert(&self, key: &str, message: Message, ttl: Option<Duration>) {
        let ttl = ttl.unwrap_or(self.default_ttl);
        let cached = Arc::new(CachedResponse { message, ttl });

        tracing::debug!(
            key = %key,
            ttl_seconds = ttl.as_secs(),
            "caching response"
        );

        self.cache.insert(key.to_string(), cached).await;
    }

    /// Extract the minimum TTL from a DNS response
    pub fn extract_ttl(message: &Message) -> Option<Duration> {
        let mut min_ttl: Option<u32> = None;

        // Check all answer records for TTL
        for record in message.answers() {
            let ttl = record.ttl();
            min_ttl = Some(min_ttl.map_or(ttl, |current| current.min(ttl)));
        }

        min_ttl.map(|secs| Duration::from_secs(secs as u64))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hickory_proto::op::Message;
    use hickory_proto::rr::{Name, RData, Record};
    use std::str::FromStr;

    #[test]
    fn test_extract_ttl_no_answers() {
        let message = Message::new();
        assert_eq!(ResponseCache::extract_ttl(&message), None);
    }

    #[test]
    fn test_extract_ttl_single_answer() {
        let mut message = Message::new();
        let name = Name::from_str("example.com.").unwrap();
        let rdata = RData::A(std::net::Ipv4Addr::new(1, 2, 3, 4).into());
        let record = Record::from_rdata(name, 300, rdata);
        message.add_answer(record);

        assert_eq!(
            ResponseCache::extract_ttl(&message),
            Some(Duration::from_secs(300))
        );
    }

    #[test]
    fn test_extract_ttl_multiple_answers_returns_minimum() {
        let mut message = Message::new();
        let name = Name::from_str("example.com.").unwrap();

        // Add record with TTL 300
        let rdata1 = RData::A(std::net::Ipv4Addr::new(1, 2, 3, 4).into());
        let record1 = Record::from_rdata(name.clone(), 300, rdata1);
        message.add_answer(record1);

        // Add record with TTL 100 (minimum)
        let rdata2 = RData::A(std::net::Ipv4Addr::new(5, 6, 7, 8).into());
        let record2 = Record::from_rdata(name.clone(), 100, rdata2);
        message.add_answer(record2);

        // Add record with TTL 500
        let rdata3 = RData::A(std::net::Ipv4Addr::new(9, 10, 11, 12).into());
        let record3 = Record::from_rdata(name, 500, rdata3);
        message.add_answer(record3);

        // Should return the minimum TTL
        assert_eq!(
            ResponseCache::extract_ttl(&message),
            Some(Duration::from_secs(100))
        );
    }
}
