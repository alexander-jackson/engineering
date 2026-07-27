use std::collections::HashSet;
use std::sync::Arc;

use color_eyre::eyre::Result;
use sqlx::PgPool;
use tokio::sync::RwLock;

use crate::persistence::DomainEventType;

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
pub trait BlocklistBackend: Send + Sync + Unpin {
    async fn read(&self) -> Result<HashSet<String>>;
    async fn update(&self, domain: &str, state: DomainEventType) -> Result<()>;
}

#[derive(Clone)]
pub struct PostgresBlocklistBackend {
    pool: PgPool,
}

impl PostgresBlocklistBackend {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait::async_trait]
impl BlocklistBackend for PostgresBlocklistBackend {
    async fn read(&self) -> Result<HashSet<String>> {
        let mut tx = self.pool.begin().await?;

        let domains = crate::persistence::select_blocked_domains(&mut tx).await?;

        tx.commit().await?;

        Ok(domains)
    }

    async fn update(&self, domain: &str, state: DomainEventType) -> Result<()> {
        let mut tx = self.pool.begin().await?;

        let domain_uid = crate::persistence::insert_domain(&mut tx, domain).await?;
        crate::persistence::insert_domain_event(&mut tx, domain_uid, state).await?;

        tx.commit().await?;

        Ok(())
    }
}

/// Manages domain blocklist loaded from external source
#[derive(Clone)]
pub struct BlocklistManager<B: BlocklistBackend = PostgresBlocklistBackend> {
    backend: B,
    blocklist: Arc<RwLock<StaticBlocklist>>,
}

impl<B: BlocklistBackend> BlocklistManager<B> {
    /// Create a new blocklist manager
    pub async fn new(backend: B) -> Result<Self> {
        let domains = backend.read().await?;
        let blocklist = StaticBlocklist::new(domains);

        let manager = Self {
            backend,
            blocklist: Arc::new(RwLock::new(blocklist)),
        };

        Ok(manager)
    }

    pub async fn read(&self) -> Result<HashSet<String>> {
        self.backend.read().await
    }

    pub async fn update(&self, domain: &str, state: DomainEventType) -> Result<()> {
        self.backend.update(domain, state).await?;

        // Refresh the blocklist after updating
        let domains = self.backend.read().await?;
        let count = domains.len();

        tracing::info!(count, "blocklist refreshed successfully");

        // Update the blocklist atomically
        *self.blocklist.write().await = StaticBlocklist::new(domains);

        Ok(())
    }
}

#[async_trait::async_trait]
impl<B: BlocklistBackend> Blocklist for BlocklistManager<B> {
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
}
