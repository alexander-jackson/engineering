use std::collections::HashSet;
use std::sync::Arc;

use color_eyre::eyre::Result;
use sqlx::PgPool;
use tokio::sync::RwLock;

use crate::persistence::DomainEventType;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BlockSource {
    Explicit,
    Remote,
}

#[async_trait::async_trait]
pub trait Blocklist: Send + Sync + Unpin {
    async fn is_blocked(&self, domain: &str) -> Option<BlockSource>;
}

#[derive(Clone, Debug, Default)]
pub struct DomainSet {
    domains: HashSet<String>,
}

impl DomainSet {
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

/// Manages the explicit (user-managed) domain blocklist loaded from `backend`, alongside a
/// remote blocklist (e.g. HaGeZi) that is fetched once at startup and does not change at runtime.
#[derive(Clone)]
pub struct BlocklistManager<B: BlocklistBackend = PostgresBlocklistBackend> {
    backend: B,
    explicit: Arc<RwLock<DomainSet>>,
    remote: Arc<DomainSet>,
}

impl<B: BlocklistBackend> BlocklistManager<B> {
    /// Create a new blocklist manager, backed by `backend` for the explicit blocklist and
    /// `remote_domains` for the (static, pre-fetched) remote blocklist.
    pub async fn new(backend: B, remote_domains: HashSet<String>) -> Result<Self> {
        let domains = backend.read().await?;
        let explicit = DomainSet::new(domains);
        let remote = DomainSet::new(remote_domains);

        let manager = Self {
            backend,
            explicit: Arc::new(RwLock::new(explicit)),
            remote: Arc::new(remote),
        };

        Ok(manager)
    }

    pub async fn read(&self) -> Result<HashSet<String>> {
        self.backend.read().await
    }

    pub async fn update(&self, domain: &str, state: DomainEventType) -> Result<()> {
        self.backend.update(domain, state).await?;

        // Refresh the explicit blocklist after updating
        let domains = self.backend.read().await?;
        let count = domains.len();

        tracing::info!(count, "explicit blocklist refreshed successfully");

        // Update the blocklist atomically
        *self.explicit.write().await = DomainSet::new(domains);

        Ok(())
    }
}

#[async_trait::async_trait]
impl<B: BlocklistBackend> Blocklist for BlocklistManager<B> {
    #[tracing::instrument(skip(self))]
    async fn is_blocked(&self, domain: &str) -> Option<BlockSource> {
        if self.explicit.read().await.is_blocked(domain) {
            return Some(BlockSource::Explicit);
        }

        if self.remote.is_blocked(domain) {
            return Some(BlockSource::Remote);
        }

        None
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use color_eyre::eyre::Result;

    use crate::blocklist::{BlockSource, Blocklist, BlocklistBackend, BlocklistManager, DomainSet};
    use crate::persistence::DomainEventType;

    #[derive(Clone, Default)]
    struct InMemoryBackend {
        domains: HashSet<String>,
    }

    #[async_trait::async_trait]
    impl BlocklistBackend for InMemoryBackend {
        async fn read(&self) -> Result<HashSet<String>> {
            Ok(self.domains.clone())
        }

        async fn update(&self, _: &str, _: DomainEventType) -> Result<()> {
            unimplemented!("not exercised by these tests")
        }
    }

    #[tokio::test]
    async fn distinguishes_explicit_and_remote_block_sources() {
        let backend = InMemoryBackend {
            domains: HashSet::from(["explicit.com".to_string()]),
        };
        let remote_domains = HashSet::from(["remote.com".to_string()]);

        let manager = BlocklistManager::new(backend, remote_domains)
            .await
            .unwrap();

        assert_eq!(
            manager.is_blocked("explicit.com").await,
            Some(BlockSource::Explicit)
        );
        assert_eq!(
            manager.is_blocked("remote.com").await,
            Some(BlockSource::Remote)
        );
        assert_eq!(manager.is_blocked("allowed.com").await, None);
    }

    #[test]
    fn empty_blocklist_allows_domains() {
        let blocklist = DomainSet::default();

        assert!(!blocklist.is_blocked("example.com"));
        assert!(!blocklist.is_blocked("sub.example.com"));
    }

    #[test]
    fn can_block_specific_domains() {
        let mut domains = HashSet::new();
        domains.insert("example.com".to_string());

        let blocklist = DomainSet::new(domains);

        assert!(blocklist.is_blocked("example.com"));
    }

    #[test]
    fn can_block_subdomains() {
        let mut domains = HashSet::new();
        domains.insert("example.com".to_string());

        let blocklist = DomainSet::new(domains);

        assert!(blocklist.is_blocked("sub.example.com"));
        assert!(blocklist.is_blocked("deep.sub.example.com"));
    }

    #[test]
    fn allows_non_blocked_domains() {
        let mut domains = HashSet::new();
        domains.insert("example.com".to_string());

        let blocklist = DomainSet::new(domains);

        assert!(!blocklist.is_blocked("other.com"));
        assert!(!blocklist.is_blocked("example.org"));
    }
}
