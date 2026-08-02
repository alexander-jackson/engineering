use std::collections::HashSet;
use std::sync::Arc;
use std::time::Duration;

use color_eyre::eyre::{Context, Result};
use foundation_configuration::ExternalBytes;
use foundation_recurring_job::{Job, Schedule};
use tokio::sync::RwLock;

#[async_trait::async_trait]
pub trait Blocklist: Send + Sync + Unpin {
    async fn is_blocked(&self, domain: &str) -> bool;
}

#[derive(Clone, Debug, Default)]
pub struct StaticBlocklist {
    domains: HashSet<String>,
}

impl StaticBlocklist {
    fn new(domains: HashSet<String>) -> Self {
        Self { domains }
    }

    fn parse(content: &str) -> Self {
        let domains = content
            .lines()
            .filter_map(|line| {
                let sanitised = line.trim();

                if sanitised.is_empty() || sanitised.starts_with('#') {
                    None
                } else {
                    Some(sanitised.to_lowercase())
                }
            })
            .collect();

        Self::new(domains)
    }

    fn is_blocked(&self, domain: &str) -> bool {
        let normalized = domain.trim_end_matches('.').to_lowercase();

        if self.domains.contains(&normalized) {
            tracing::debug!(domain = %normalized, "exact match on blocklist");

            return true;
        }

        // Check subdomain matches
        let parts: Vec<&str> = normalized.split('.').collect();

        for i in 1..parts.len() {
            let parent = parts[i..].join(".");

            if self.domains.contains(&parent) {
                tracing::debug!(
                    domain = %normalized,
                    parent = %parent,
                    "subdomain match on blocklist"
                );

                return true;
            }
        }

        false
    }
}

#[async_trait::async_trait]
pub trait BlocklistResolver: Send + Sync + Unpin {
    async fn resolve(&self) -> Result<StaticBlocklist>;
}

#[derive(Clone)]
pub struct ExternalBytesBlocklistResolver {
    source: ExternalBytes,
}

impl ExternalBytesBlocklistResolver {
    pub fn new(source: ExternalBytes) -> Self {
        Self { source }
    }
}

#[async_trait::async_trait]
impl BlocklistResolver for ExternalBytesBlocklistResolver {
    async fn resolve(&self) -> Result<StaticBlocklist> {
        let data = self
            .source
            .resolve()
            .await
            .wrap_err("failed to load blocklist")?;

        let content = std::str::from_utf8(&data).wrap_err("blocklist is not valid UTF-8")?;

        Ok(StaticBlocklist::parse(content))
    }
}

/// Manages domain blocklist loaded from external source
#[derive(Clone)]
pub struct BlocklistManager<R: BlocklistResolver = ExternalBytesBlocklistResolver> {
    interval: Duration,
    resolver: R,
    blocklist: Arc<RwLock<StaticBlocklist>>,
}

impl<R: BlocklistResolver> BlocklistManager<R> {
    /// Create a new blocklist manager
    pub async fn new(interval: Duration, resolver: R) -> Result<Self> {
        let manager = Self {
            interval,
            resolver,
            blocklist: Default::default(),
        };

        Ok(manager)
    }
}

impl Job for BlocklistManager {
    const NAME: &'static str = "blocklist-manager";

    fn schedule(&self) -> Schedule {
        Schedule::interval(self.interval)
    }

    async fn run(&self) -> Result<()> {
        tracing::info!("refreshing blocklist");

        let blocklist = self.resolver.resolve().await?;
        let count = blocklist.domains.len();

        // Update the blocklist atomically
        *self.blocklist.write().await = blocklist;

        tracing::info!(count, "blocklist refreshed successfully");

        Ok(())
    }
}

#[async_trait::async_trait]
impl Blocklist for BlocklistManager {
    #[tracing::instrument(skip(self))]
    async fn is_blocked(&self, domain: &str) -> bool {
        self.blocklist.read().await.is_blocked(domain)
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use crate::blocklist::StaticBlocklist;

    #[test]
    fn empty_blocklist_allows_domains() {
        let blocklist = StaticBlocklist::default();

        assert!(!blocklist.is_blocked("example.com"));
        assert!(!blocklist.is_blocked("sub.example.com"));
    }

    #[test]
    fn can_block_specific_domains() {
        let mut domains = HashSet::new();
        domains.insert("example.com".to_string());

        let blocklist = StaticBlocklist::new(domains);

        assert!(blocklist.is_blocked("example.com"));
    }

    #[test]
    fn can_block_subdomains() {
        let mut domains = HashSet::new();
        domains.insert("example.com".to_string());

        let blocklist = StaticBlocklist::new(domains);

        assert!(blocklist.is_blocked("sub.example.com"));
        assert!(blocklist.is_blocked("deep.sub.example.com"));
    }

    #[test]
    fn allows_non_blocked_domains() {
        let mut domains = HashSet::new();
        domains.insert("example.com".to_string());

        let blocklist = StaticBlocklist::new(domains);

        assert!(!blocklist.is_blocked("other.com"));
        assert!(!blocklist.is_blocked("example.org"));
    }

    #[test]
    fn can_parse_blocklist_content() {
        let content = r#"
            # This is a comment
            example.com
            test.org

            # Another comment
            sub.domain.net

            # Domain which we used to block
            # google.com
        "#;

        let blocklist = StaticBlocklist::parse(content);

        let expected = HashSet::from([
            "example.com".to_string(),
            "test.org".to_string(),
            "sub.domain.net".to_string(),
        ]);

        assert_eq!(blocklist.domains, expected);
    }
}
