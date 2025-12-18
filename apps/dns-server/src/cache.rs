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
        message
            .answers()
            .iter()
            .fold(None, |min_ttl, record| {
                let ttl = record.ttl();

                Some(min_ttl.map_or(ttl, |current: u32| current.min(ttl)))
            })
            .map(|secs| Duration::from_secs(secs as u64))
    }
}

#[cfg(test)]
mod tests {
    use std::net::Ipv4Addr;
    use std::str::FromStr;
    use std::time::Duration;

    use hickory_proto::op::Message;
    use hickory_proto::rr::{Name, RData, Record};

    use crate::cache::ResponseCache;

    fn some_record_with_ttl(name: &Name, ttl: u32) -> Record {
        let rdata = RData::A(Ipv4Addr::LOCALHOST.into());

        Record::from_rdata(name.clone(), ttl, rdata)
    }

    #[test]
    fn default_messages_have_no_time_to_live() {
        let message = Message::new();

        assert_eq!(ResponseCache::extract_ttl(&message), None);
    }

    #[test]
    fn can_extract_ttl_for_single_record_messages() {
        let mut message = Message::new();
        let name = Name::from_str("example.com.").unwrap();

        message.add_answer(some_record_with_ttl(&name, 300));

        assert_eq!(
            ResponseCache::extract_ttl(&message),
            Some(Duration::from_secs(300))
        );
    }

    #[test]
    fn can_extract_minimum_ttl_for_multiple_record_messages() {
        let mut message = Message::new();
        let name = Name::from_str("example.com.").unwrap();

        message.add_answer(some_record_with_ttl(&name, 300));
        message.add_answer(some_record_with_ttl(&name, 100));
        message.add_answer(some_record_with_ttl(&name, 500));

        assert_eq!(
            ResponseCache::extract_ttl(&message),
            Some(Duration::from_secs(100))
        );
    }
}
