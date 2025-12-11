use std::collections::HashSet;
use std::sync::Arc;

use color_eyre::eyre::{Context, Result};
use tokio::sync::RwLock;

use crate::config::BlocklistConfig;

/// Manages domain blocklist loaded from external source
#[derive(Clone)]
pub struct BlocklistManager {
    config: BlocklistConfig,
    domains: Arc<RwLock<HashSet<String>>>,
}

impl BlocklistManager {
    /// Create a new blocklist manager
    pub fn new(config: BlocklistConfig) -> Self {
        Self {
            config,
            domains: Arc::new(RwLock::new(HashSet::new())),
        }
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

        let content = String::from_utf8(data).wrap_err("blocklist is not valid UTF-8")?;

        // Parse domains (one per line)
        let domains: HashSet<String> = content
            .lines()
            .map(|line| line.trim())
            .filter(|line| !line.is_empty() && !line.starts_with('#'))
            .map(|domain| domain.to_lowercase())
            .collect();

        let count = domains.len();

        // Update the blocklist atomically
        let mut guard = self.domains.write().await;
        *guard = domains;

        tracing::info!(count, "blocklist refreshed successfully");

        Ok(())
    }

    /// Check if a domain is blocked
    #[tracing::instrument(skip(self))]
    pub async fn is_blocked(&self, domain: &str) -> bool {
        let normalized = domain.trim_end_matches('.').to_lowercase();

        let guard = self.domains.read().await;

        // Check exact match
        if guard.contains(&normalized) {
            tracing::debug!(domain = %normalized, "exact match on blocklist");
            return true;
        }

        // Check subdomain matches (e.g., "ads.example.com" blocked if "example.com" in list)
        let parts: Vec<&str> = normalized.split('.').collect();
        for i in 1..parts.len() {
            let parent = parts[i..].join(".");
            if guard.contains(&parent) {
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
