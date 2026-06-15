use std::time::Instant;

use hickory_proto::op::{Header, HeaderCounts, Message, MessageType, OpCode, ResponseCode};
use hickory_server::net::runtime::Time;
use hickory_server::server::{Request, RequestHandler, ResponseHandler, ResponseInfo};
use hickory_server::zone_handler::MessageResponseBuilder;
use opentelemetry::KeyValue;

use crate::blocklist::BlocklistSource;
use crate::cache::ResponseCache;
use crate::server::DnsServerMetrics;
use crate::upstream::Upstream;

#[derive(Clone)]
pub struct DnsRequestHandler<U, B> {
    upstream: U,
    blocklist: B,
    cache: ResponseCache,
    metrics: DnsServerMetrics,
}

impl<U: Upstream, B: BlocklistSource> DnsRequestHandler<U, B> {
    pub fn new(upstream: U, blocklist: B, cache: ResponseCache, metrics: DnsServerMetrics) -> Self {
        Self {
            upstream,
            blocklist,
            cache,
            metrics,
        }
    }
}

fn make_response_info(request: &Request) -> ResponseInfo {
    ResponseInfo::from(Header {
        metadata: request.metadata,
        counts: HeaderCounts::default(),
    })
}

#[async_trait::async_trait]
impl<U, B> RequestHandler for DnsRequestHandler<U, B>
where
    U: Upstream + 'static,
    B: BlocklistSource + 'static,
{
    async fn handle_request<R: ResponseHandler, T: Time>(
        &self,
        request: &Request,
        mut response_handle: R,
    ) -> ResponseInfo {
        self.metrics.requests.add(1, &[]);
        let start = Instant::now();

        let request_info = match request.request_info() {
            Ok(info) => info,
            Err(e) => {
                tracing::error!(error = ?e, "failed to parse request");

                let response = MessageResponseBuilder::from_message_request(request)
                    .error_msg(&request.metadata, ResponseCode::FormErr);

                let result = match response_handle.send_response(response).await {
                    Ok(info) => info,
                    Err(e) => {
                        tracing::error!(error = ?e, "failed to send error response");
                        make_response_info(request)
                    }
                };
                self.metrics.request_duration.record(
                    start.elapsed().as_millis() as f64,
                    &[KeyValue::new("type", "parse-error")],
                );
                return result;
            }
        };

        let domain_name = request_info.query.name().to_string();
        tracing::info!(
            name = %domain_name,
            query_type = ?request_info.query.query_type(),
            src = %request.src(),
            "received DNS query"
        );

        if self.blocklist.is_blocked(&domain_name).await {
            tracing::info!(
                name = %domain_name,
                src = %request.src(),
                "blocked domain query"
            );

            let attrs = [KeyValue::new("type", "explicit-block")];
            self.metrics.responses.add(1, &attrs);

            let response = MessageResponseBuilder::from_message_request(request)
                .error_msg(&request.metadata, ResponseCode::Refused);

            let result = match response_handle.send_response(response).await {
                Ok(info) => info,
                Err(e) => {
                    tracing::error!(error = ?e, "failed to send blocked response");
                    make_response_info(request)
                }
            };
            self.metrics
                .request_duration
                .record(start.elapsed().as_millis() as f64, &attrs);
            return result;
        }

        let cache_key = format!("{}:{:?}", domain_name, request_info.query.query_type());

        if let Some(cached_response) = self.cache.get(&cache_key).await {
            let is_negative = cached_response.metadata.response_code == ResponseCode::ServFail;
            let type_label = if is_negative {
                "negative-cache-hit"
            } else {
                "cache-hit"
            };
            tracing::debug!(
                name = %domain_name,
                src = %request.src(),
                negative = is_negative,
                "returning cached response"
            );

            let attrs = [KeyValue::new("type", type_label)];
            self.metrics.responses.add(1, &attrs);

            let mut metadata = cached_response.metadata;
            metadata.id = request.metadata.id;

            let response = MessageResponseBuilder::from_message_request(request).build(
                metadata,
                cached_response.answers.iter(),
                cached_response.authorities.iter(),
                &[],
                cached_response.additionals.iter(),
            );
            let result = match response_handle.send_response(response).await {
                Ok(info) => info,
                Err(e) => {
                    tracing::error!(error = ?e, "failed to send cached response");
                    make_response_info(request)
                }
            };
            self.metrics
                .request_duration
                .record(start.elapsed().as_millis() as f64, &attrs);
            return result;
        }

        let mut request_message = Message::new(
            request.metadata.id,
            MessageType::Query,
            request.metadata.op_code,
        );
        request_message.metadata.recursion_desired = request.metadata.recursion_desired;
        request_message.add_query(request_info.query.original().clone());

        let upstream_start = Instant::now();
        let (response_message, response_type) = match self.upstream.resolve(&request_message).await
        {
            Ok(response) => {
                tracing::debug!(
                    src = %request.src(),
                    answer_count = response.answers.len(),
                    "upstream resolution successful"
                );

                let ttl = ResponseCache::extract_ttl(&response);
                self.cache.insert(&cache_key, response.clone(), ttl).await;

                (response, "upstream")
            }
            Err(e) => {
                tracing::warn!(error = ?e, src = %request.src(), "upstream resolution failed");

                let mut error_msg = Message::new(
                    request_message.metadata.id,
                    MessageType::Response,
                    OpCode::Query,
                );
                error_msg.metadata.response_code = ResponseCode::ServFail;
                error_msg.add_query(request_info.query.original().clone());
                self.cache
                    .insert(&cache_key, error_msg.clone(), Some(self.cache.error_ttl()))
                    .await;
                (error_msg, "upstream-error")
            }
        };
        self.metrics
            .upstream_duration
            .record(upstream_start.elapsed().as_millis() as f64, &[]);
        let attrs = [KeyValue::new("type", response_type)];
        self.metrics.responses.add(1, &attrs);

        let response = MessageResponseBuilder::from_message_request(request).build(
            response_message.metadata,
            response_message.answers.iter(),
            response_message.authorities.iter(),
            &[],
            response_message.additionals.iter(),
        );

        let result = match response_handle.send_response(response).await {
            Ok(info) => info,
            Err(e) => {
                tracing::error!(error = ?e, "failed to send response");
                make_response_info(request)
            }
        };
        self.metrics
            .request_duration
            .record(start.elapsed().as_millis() as f64, &attrs);
        result
    }
}

#[cfg(test)]
mod tests {
    use std::net::Ipv4Addr;
    use std::str::FromStr;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use hickory_net::xfer::Protocol;
    use hickory_proto::op::{
        Header, HeaderCounts, Message, MessageType, OpCode, Query, ResponseCode,
    };
    use hickory_proto::rr::{DNSClass, Name, RData, Record, RecordType};
    use hickory_proto::serialize::binary::BinEncodable;
    use hickory_server::net::NetError;
    use hickory_server::net::runtime::TokioTime;
    use hickory_server::server::{Request, RequestHandler, ResponseInfo};
    use hickory_server::zone_handler::MessageResponse;

    use crate::blocklist::BlocklistSource;
    use crate::cache::ResponseCache;
    use crate::config::CacheConfig;
    use crate::server::DnsServerMetrics;
    use crate::upstream::Upstream;

    use super::DnsRequestHandler;

    fn test_metrics() -> DnsServerMetrics {
        DnsServerMetrics::new(&opentelemetry::global::meter("test"))
    }

    fn test_cache() -> ResponseCache {
        ResponseCache::new(&CacheConfig {
            max_entries: 100,
            default_ttl_seconds: 60,
            error_ttl_seconds: 10,
        })
    }

    fn make_test_request(name: &str) -> Request {
        let name = Name::from_str(name).unwrap();
        let mut query = Query::new();
        query.set_name(name);
        query.set_query_type(RecordType::A);
        query.set_query_class(DNSClass::IN);

        let mut message = Message::new(1, MessageType::Query, OpCode::Query);
        message.add_query(query);

        let bytes = message.to_bytes().unwrap();
        Request::from_bytes(bytes, "127.0.0.1:1234".parse().unwrap(), Protocol::Udp).unwrap()
    }

    fn make_success_message() -> Message {
        let name = Name::from_str("example.com.").unwrap();
        let rdata = RData::A(Ipv4Addr::new(93, 184, 216, 34).into());
        let record = Record::from_rdata(name, 300, rdata);

        let mut msg = Message::new(1, MessageType::Response, OpCode::Query);
        msg.add_answer(record);
        msg
    }

    fn make_servfail_message() -> Message {
        let mut msg = Message::new(1, MessageType::Response, OpCode::Query);
        msg.metadata.response_code = ResponseCode::ServFail;
        msg
    }

    #[derive(Clone)]
    struct AlwaysBlocked;

    #[async_trait::async_trait]
    impl BlocklistSource for AlwaysBlocked {
        async fn is_blocked(&self, _: &str) -> bool {
            true
        }
    }

    #[derive(Clone)]
    struct NeverBlocked;

    #[async_trait::async_trait]
    impl BlocklistSource for NeverBlocked {
        async fn is_blocked(&self, _: &str) -> bool {
            false
        }
    }

    #[derive(Clone)]
    struct SuccessUpstream(Message);

    #[async_trait::async_trait]
    impl Upstream for SuccessUpstream {
        async fn resolve(&self, _: &Message) -> color_eyre::eyre::Result<Message> {
            Ok(self.0.clone())
        }
    }

    #[derive(Clone)]
    struct FailingUpstream(Arc<AtomicUsize>);

    #[async_trait::async_trait]
    impl Upstream for FailingUpstream {
        async fn resolve(&self, _: &Message) -> color_eyre::eyre::Result<Message> {
            self.0.fetch_add(1, Ordering::SeqCst);
            Err(color_eyre::eyre::eyre!("upstream error"))
        }
    }

    #[derive(Clone, Default)]
    struct CapturingResponseHandler;

    #[async_trait::async_trait]
    impl hickory_server::server::ResponseHandler for CapturingResponseHandler {
        async fn send_response<'a>(
            &mut self,
            response: MessageResponse<
                '_,
                'a,
                impl Iterator<Item = &'a Record> + Send + 'a,
                impl Iterator<Item = &'a Record> + Send + 'a,
                impl Iterator<Item = &'a Record> + Send + 'a,
                impl Iterator<Item = &'a Record> + Send + 'a,
            >,
        ) -> Result<ResponseInfo, NetError> {
            let metadata = *response.metadata();
            Ok(ResponseInfo::from(Header {
                metadata,
                counts: HeaderCounts::default(),
            }))
        }
    }

    #[tokio::test]
    async fn blocked_domain_returns_refused() {
        let handler = DnsRequestHandler::new(
            SuccessUpstream(make_success_message()),
            AlwaysBlocked,
            test_cache(),
            test_metrics(),
        );
        let request = make_test_request("blocked.com.");

        let info = handler
            .handle_request::<_, TokioTime>(&request, CapturingResponseHandler)
            .await;

        assert_eq!(info.response_code, ResponseCode::Refused);
    }

    #[tokio::test]
    async fn cache_hit_skips_upstream() {
        let cache = test_cache();
        let call_count = Arc::new(AtomicUsize::new(0));

        cache
            .insert("example.com.:A", make_success_message(), None)
            .await;

        let handler = DnsRequestHandler::new(
            FailingUpstream(call_count.clone()),
            NeverBlocked,
            cache,
            test_metrics(),
        );
        let request = make_test_request("example.com.");

        let info = handler
            .handle_request::<_, TokioTime>(&request, CapturingResponseHandler)
            .await;

        assert_eq!(info.response_code, ResponseCode::NoError);
        assert_eq!(
            call_count.load(Ordering::SeqCst),
            0,
            "upstream should not be called on cache hit"
        );
    }

    #[tokio::test]
    async fn negative_cache_hit_returns_servfail_without_calling_upstream() {
        let cache = test_cache();

        cache
            .insert("example.com.:A", make_servfail_message(), None)
            .await;

        let handler = DnsRequestHandler::new(
            SuccessUpstream(make_success_message()),
            NeverBlocked,
            cache,
            test_metrics(),
        );
        let request = make_test_request("example.com.");

        let info = handler
            .handle_request::<_, TokioTime>(&request, CapturingResponseHandler)
            .await;

        assert_eq!(
            info.response_code,
            ResponseCode::ServFail,
            "ServFail with a SuccessUpstream proves the cached error was served"
        );
    }

    #[tokio::test]
    async fn upstream_success_is_returned_and_cached() {
        let cache = test_cache();
        let handler = DnsRequestHandler::new(
            SuccessUpstream(make_success_message()),
            NeverBlocked,
            cache.clone(),
            test_metrics(),
        );
        let request = make_test_request("example.com.");

        let info = handler
            .handle_request::<_, TokioTime>(&request, CapturingResponseHandler)
            .await;

        assert_eq!(info.response_code, ResponseCode::NoError);
        assert!(
            cache.get("example.com.:A").await.is_some(),
            "successful response should be cached"
        );
    }

    #[tokio::test]
    async fn upstream_error_returns_servfail_and_is_cached() {
        let cache = test_cache();
        let call_count = Arc::new(AtomicUsize::new(0));
        let handler = DnsRequestHandler::new(
            FailingUpstream(call_count.clone()),
            NeverBlocked,
            cache.clone(),
            test_metrics(),
        );
        let request = make_test_request("example.com.");

        let info = handler
            .handle_request::<_, TokioTime>(&request, CapturingResponseHandler)
            .await;

        assert_eq!(info.response_code, ResponseCode::ServFail);
        assert_eq!(call_count.load(Ordering::SeqCst), 1);
        assert!(
            cache.get("example.com.:A").await.is_some(),
            "upstream error should be cached for negative caching"
        );
    }
}
