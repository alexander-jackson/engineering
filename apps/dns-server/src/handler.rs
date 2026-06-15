use std::time::Instant;

use hickory_proto::op::{Header, HeaderCounts, Message, MessageType, OpCode, ResponseCode};
use hickory_server::net::runtime::Time;
use hickory_server::server::{Request, RequestHandler, ResponseHandler, ResponseInfo};
use hickory_server::zone_handler::MessageResponseBuilder;
use opentelemetry::KeyValue;

use crate::blocklist::BlocklistManager;
use crate::cache::ResponseCache;
use crate::server::DnsServerMetrics;
use crate::upstream::UpstreamResolver;

#[derive(Clone)]
pub struct DnsRequestHandler {
    upstream: UpstreamResolver,
    blocklist: BlocklistManager,
    cache: ResponseCache,
    metrics: DnsServerMetrics,
}

impl DnsRequestHandler {
    pub fn new(
        upstream: UpstreamResolver,
        blocklist: BlocklistManager,
        cache: ResponseCache,
        metrics: DnsServerMetrics,
    ) -> Self {
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
impl RequestHandler for DnsRequestHandler {
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
