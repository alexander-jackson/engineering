use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use color_eyre::eyre::{Context, Result};
use hickory_proto::op::Message;
use hickory_proto::xfer::Protocol;
use hickory_resolver::config::{NameServerConfig, ResolverConfig, ResolverOpts};
use hickory_resolver::name_server::TokioConnectionProvider;
use hickory_resolver::{ResolveError, Resolver};

use crate::config::UpstreamConfig;

/// Upstream DNS resolver that forwards queries to Mullvad DNS
#[derive(Clone)]
pub struct UpstreamResolver {
    resolver: Arc<Resolver<TokioConnectionProvider>>,
}

impl UpstreamResolver {
    #[tracing::instrument(skip(config))]
    pub async fn new(config: &UpstreamConfig) -> Result<Self> {
        // Resolve the upstream DNS server hostname to an IP address
        let upstream_addrs: Vec<SocketAddr> =
            tokio::net::lookup_host(format!("{}:{}", config.resolver, config.port))
                .await
                .wrap_err_with(|| {
                    format!("failed to resolve upstream DNS server: {}", config.resolver)
                })?
                .collect();

        let upstream_addr = upstream_addrs.first().copied().ok_or_else(|| {
            color_eyre::eyre::eyre!("no IP addresses found for {}", config.resolver)
        })?;

        tracing::info!(
            resolver = %config.resolver,
            addr = %upstream_addr,
            "resolved upstream DNS server"
        );

        // Configure resolver to use only Mullvad DNS with HTTPS
        let mut resolver_config = ResolverConfig::new();
        let mut nameserver = NameServerConfig::new(upstream_addr, Protocol::Https);
        // Set TLS DNS name for certificate verification
        nameserver.tls_dns_name = Some(config.resolver.clone());
        resolver_config.add_name_server(nameserver);

        // Configure resolver options with timeout
        let mut opts = ResolverOpts::default();
        opts.timeout = Duration::from_secs(config.timeout_seconds);

        let resolver =
            Resolver::builder_with_config(resolver_config, TokioConnectionProvider::default())
                .with_options(opts)
                .build();

        tracing::info!(
            upstream = %config.resolver,
            protocol = "HTTPS",
            timeout_seconds = config.timeout_seconds,
            "initialized upstream DNS resolver"
        );

        Ok(Self {
            resolver: Arc::new(resolver),
        })
    }

    #[tracing::instrument(skip(self, query))]
    pub async fn resolve(&self, query: &Message) -> Result<Message, ResolveError> {
        // Extract query information for logging
        let question = &query.queries()[0];
        let name = question.name();
        let query_type = question.query_type();

        tracing::debug!(
            name = %name,
            query_type = ?query_type,
            "forwarding query to upstream"
        );

        // Perform the lookup using the Name directly (not string)
        let lookup = self.resolver.lookup(name.clone(), query_type).await?;

        // Build response message
        let mut response = Message::new();
        response.set_id(query.id());
        response.set_message_type(hickory_proto::op::MessageType::Response);
        response.set_op_code(hickory_proto::op::OpCode::Query);
        response.set_recursion_desired(query.recursion_desired());
        response.set_recursion_available(true);

        // Add the question
        response.add_query(question.clone());

        // Add answer records
        for record in lookup.records() {
            response.add_answer(record.clone());
        }

        tracing::debug!(
            name = %name,
            answer_count = response.answer_count(),
            "received upstream response"
        );

        Ok(response)
    }
}
