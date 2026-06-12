use std::sync::Arc;
use std::time::Duration;

use color_eyre::eyre::{Context, Result, eyre};
use hickory_net::xfer::Protocol;
use hickory_proto::op::{Message, MessageType, OpCode};
use hickory_resolver::Resolver;
use hickory_resolver::config::{ConnectionConfig, NameServerConfig, ResolverConfig, ResolverOpts};
use hickory_resolver::net::NetError;
use hickory_resolver::net::runtime::TokioRuntimeProvider;

use crate::config::UpstreamConfig;

#[derive(Clone)]
pub struct UpstreamResolver {
    resolver: Arc<Resolver<TokioRuntimeProvider>>,
}

impl UpstreamResolver {
    #[tracing::instrument(skip(config))]
    pub async fn new(config: &UpstreamConfig) -> Result<Self> {
        let upstream_addrs: Vec<std::net::SocketAddr> =
            tokio::net::lookup_host(format!("{}:{}", config.resolver, config.port))
                .await
                .wrap_err_with(|| {
                    format!("failed to resolve upstream DNS server: {}", config.resolver)
                })?
                .collect();

        let upstream_addr = upstream_addrs
            .first()
            .copied()
            .ok_or_else(|| eyre!("no IP addresses found for {}", config.resolver))?;

        tracing::info!(
            resolver = %config.resolver,
            addr = %upstream_addr,
            "resolved upstream DNS server"
        );

        let server_name: Arc<str> = Arc::from(config.resolver.as_str());
        let mut connection = match config.protocol {
            Protocol::Udp => ConnectionConfig::udp(),
            Protocol::Tcp => ConnectionConfig::tcp(),
            Protocol::Tls => ConnectionConfig::tls(server_name),
            Protocol::Https => ConnectionConfig::https(server_name, None),
            p => return Err(eyre!("unsupported upstream protocol: {:?}", p)),
        };
        connection.port = config.port;

        let nameserver = NameServerConfig::new(upstream_addr.ip(), true, vec![connection]);

        let resolver_config = ResolverConfig::from_parts(None, vec![], vec![nameserver]);

        let mut opts = ResolverOpts::default();
        opts.timeout = Duration::from_secs(config.timeout_seconds);

        let provider = TokioRuntimeProvider::default();

        let resolver = Resolver::builder_with_config(resolver_config, provider)
            .with_options(opts)
            .build()
            .wrap_err("failed to build upstream resolver")?;

        tracing::info!(
            upstream = %config.resolver,
            timeout_seconds = config.timeout_seconds,
            "initialized upstream DNS resolver"
        );

        Ok(Self {
            resolver: Arc::new(resolver),
        })
    }

    #[tracing::instrument(skip(self, query))]
    pub async fn resolve(&self, query: &Message) -> Result<Message, NetError> {
        let question = &query.queries[0];
        let name = question.name();
        let query_type = question.query_type();

        tracing::debug!(
            name = %name,
            query_type = ?query_type,
            "forwarding query to upstream"
        );

        let lookup = self.resolver.lookup(name.clone(), query_type).await?;

        let mut response = Message::new(query.metadata.id, MessageType::Response, OpCode::Query);
        response.metadata.recursion_desired = query.metadata.recursion_desired;
        response.metadata.recursion_available = true;

        response.add_query(question.clone());

        for record in lookup.answers() {
            response.add_answer(record.clone());
        }

        tracing::debug!(
            name = %name,
            answer_count = response.answers.len(),
            "received upstream response"
        );

        Ok(response)
    }
}

#[cfg(test)]
mod tests {
    use std::net::{IpAddr, Ipv4Addr, SocketAddrV4};
    use std::str::FromStr;

    use color_eyre::eyre::Result;
    use dns_mock_server::Server;
    use hickory_net::xfer::Protocol;
    use hickory_proto::op::{Message, MessageType, OpCode, Query};
    use hickory_proto::rr::{DNSClass, Name, RData, RecordType};
    use tokio::net::UdpSocket;

    use crate::config::UpstreamConfig;
    use crate::upstream::UpstreamResolver;

    #[tokio::test]
    async fn can_resolve_upstream_records() -> Result<()> {
        let upstream_addr = Ipv4Addr::new(93, 184, 216, 34);

        let mut mock_server = Server::default();
        mock_server.add_records("example.com.", vec![IpAddr::V4(upstream_addr)])?;

        let socket = UdpSocket::bind(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 0)).await?;
        let addr = socket.local_addr()?;

        tokio::spawn(async move {
            mock_server.start(socket).await.unwrap();
        });

        let config = UpstreamConfig {
            resolver: addr.ip().to_string(),
            port: addr.port(),
            protocol: Protocol::Udp,
            timeout_seconds: 5,
        };

        let resolver = UpstreamResolver::new(&config).await?;

        let name = Name::from_str("example.com.")?;
        let query_type = RecordType::A;
        let query_class = DNSClass::IN;

        let mut query = Query::new();
        query.set_name(name);
        query.set_query_type(query_type);
        query.set_query_class(query_class);

        let mut message = Message::new(0, MessageType::Query, OpCode::Query);
        message.add_query(query);

        let resolved = resolver.resolve(&message).await?;

        let answers: Vec<_> = resolved
            .answers
            .iter()
            .filter_map(|record| match &record.data {
                RData::A(a) => Some(a.0),
                _ => None,
            })
            .collect();

        assert_eq!(answers, vec![upstream_addr]);

        Ok(())
    }
}
