use std::collections::HashSet;
use std::sync::Arc;
use std::time::Duration;

use color_eyre::eyre::{Context, Result};
use tokio::sync::RwLock;

use crate::config::BlocklistConfig;

#[derive(Clone, Debug, Default)]
struct Blocklist {
    domains: HashSet<String>,
}

impl Blocklist {
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

/// Manages domain blocklist loaded from external source
#[derive(Clone)]
pub struct BlocklistManager {
    config: BlocklistConfig,
    blocklist: Arc<RwLock<Blocklist>>,
}

impl BlocklistManager {
    /// Create a new blocklist manager
    pub async fn new(config: BlocklistConfig) -> Result<Self> {
        let manager = Self {
            config,
            blocklist: Default::default(),
        };

        manager.refresh().await?;
        manager.spawn_refresh_task();

        Ok(manager)
    }

    pub fn spawn_refresh_task(&self) {
        let manager = self.clone();
        let interval = Duration::from_secs(self.config.refresh_interval_seconds);

        tokio::spawn(async move {
            let mut ticker = tokio::time::interval(interval);

            loop {
                ticker.tick().await;

                if let Err(e) = manager.refresh().await {
                    tracing::error!(error = ?e, "failed to refresh blocklist");
                }
            }
        });
    }

    /// Refresh the blocklist from external source
    #[tracing::instrument(skip(self))]
    pub async fn refresh(&self) -> Result<()> {
        tracing::info!(
            source = ?self.config.source,
            "refreshing blocklist"
        );

        // Load blocklist from external source
        let data = self
            .config
            .source
            .resolve()
            .await
            .wrap_err("failed to load blocklist")?;

        let content = str::from_utf8(&data).wrap_err("blocklist is not valid UTF-8")?;

        let blocklist = Blocklist::parse(content);
        let count = blocklist.domains.len();

        // Update the blocklist atomically
        *self.blocklist.write().await = blocklist;

        tracing::info!(count, "blocklist refreshed successfully");

        Ok(())
    }

    #[tracing::instrument(skip(self))]
    pub async fn is_blocked(&self, domain: &str) -> bool {
        self.blocklist.read().await.is_blocked(domain)
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use crate::blocklist::Blocklist;

    #[test]
    fn empty_blocklist_allows_domains() {
        let blocklist = Blocklist::default();

        assert!(!blocklist.is_blocked("example.com"));
        assert!(!blocklist.is_blocked("sub.example.com"));
    }

    #[test]
    fn can_block_specific_domains() {
        let mut domains = HashSet::new();
        domains.insert("example.com".to_string());

        let blocklist = Blocklist::new(domains);

        assert!(blocklist.is_blocked("example.com"));
    }

    #[test]
    fn can_block_subdomains() {
        let mut domains = HashSet::new();
        domains.insert("example.com".to_string());

        let blocklist = Blocklist::new(domains);

        assert!(blocklist.is_blocked("sub.example.com"));
        assert!(blocklist.is_blocked("deep.sub.example.com"));
    }

    #[test]
    fn allows_non_blocked_domains() {
        let mut domains = HashSet::new();
        domains.insert("example.com".to_string());

        let blocklist = Blocklist::new(domains);

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

        let blocklist = Blocklist::parse(content);

        let expected = HashSet::from([
            "example.com".to_string(),
            "test.org".to_string(),
            "sub.domain.net".to_string(),
        ]);

        assert_eq!(blocklist.domains, expected);
    }
}
