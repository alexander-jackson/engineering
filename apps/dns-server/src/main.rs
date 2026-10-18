use std::collections::HashSet;
use std::sync::Arc;
use std::time::Duration;

use color_eyre::eyre::Result;
use foundation_shutdown::ShutdownCoordinator;
use foundation_templating::TemplateEngine;
use tokio::net::TcpListener;

mod blocklist;
mod cache;
mod classifier;
mod config;
mod handler;
mod http_server;
mod persistence;
mod remote_blocklist;
mod server;
mod templates;
mod uid;
mod upstream;

use crate::blocklist::{BlocklistManager, PostgresBlocklistBackend};
use crate::cache::ResponseCache;
use crate::classifier::{
    ClassifierMetrics, Classify, DomainClassifier, OpenRouterDecider, PostgresDecisionStore,
    ReqwestFetcher,
};
use crate::config::Configuration;
use crate::server::{DnsServer, DnsServerMetrics};
use crate::upstream::UpstreamResolver;

#[tokio::main]
async fn main() -> Result<()> {
    let (config, pool) = foundation_init::run_with_bootstrap::<Configuration>().await?;

    tracing::info!(
        upstream = %config.upstream.resolver,
        "dns server initialized"
    );

    let backend = PostgresBlocklistBackend::new(pool.clone());

    let remote_domains = match &config.remote_blocklist {
        Some(remote_config) => remote_blocklist::fetch(&remote_config.url).await?,
        None => HashSet::new(),
    };

    let blocklist_manager = BlocklistManager::new(backend.clone(), remote_domains).await?;
    let upstream = UpstreamResolver::new(&config.upstream).await?;
    let cache = ResponseCache::new(&config.cache);

    let meter = opentelemetry::global::meter("dns-server");

    let decision_store = PostgresDecisionStore::new(pool.clone());

    let classifier: Option<Arc<dyn Classify>> = match &config.classifier {
        Some(classifier_config) => {
            tracing::info!(config = ?classifier_config, "setting up the classifier for new domains");

            let fetcher =
                ReqwestFetcher::new(Duration::from_secs(5), classifier_config.max_content_chars)?;
            let decider =
                OpenRouterDecider::new(&classifier_config.api_key, &classifier_config.model)?;
            let store = decision_store.clone();
            let wait = Duration::from_secs(classifier_config.timeout_seconds);

            Some(Arc::new(
                DomainClassifier::new(
                    fetcher,
                    decider,
                    store,
                    wait,
                    ClassifierMetrics::new(&meter),
                )
                .await?,
            ))
        }
        None => None,
    };

    let metrics = DnsServerMetrics::new(&meter);

    let addr = (config.server.dns.host, config.server.dns.port);
    let dns_listener = TcpListener::bind(addr).await?;

    let addr = (config.server.http.host, config.server.http.port);
    let http_listener = TcpListener::bind(addr).await?;

    let dns_server = DnsServer::new(
        dns_listener,
        upstream,
        blocklist_manager.clone(),
        cache,
        metrics,
        classifier,
    )
    .await?;

    let template_engine = TemplateEngine::new()?;
    let http_server = crate::http_server::build(
        blocklist_manager.clone(),
        Arc::new(decision_store),
        template_engine,
        http_listener,
    );

    ShutdownCoordinator::new()
        .with_task(dns_server)
        .with_task(http_server)
        .run()
        .await?;

    Ok(())
}
