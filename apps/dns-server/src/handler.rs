use std::time::Instant;

use hickory_proto::op::{Message, MessageType, OpCode, ResponseCode};
use hickory_server::authority::MessageResponseBuilder;
use hickory_server::server::{Request, RequestHandler, ResponseHandler, ResponseInfo};
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

#[async_trait::async_trait]
impl RequestHandler for DnsRequestHandler {
    async fn handle_request<R: ResponseHandler>(
        &self,
        request: &Request,
        mut response_handle: R,
    ) -> ResponseInfo {
        self.metrics.requests.add(1, &[]);
        let start = Instant::now();

        // Get request info to extract the query
        let request_info = match request.request_info() {
            Ok(info) => info,
            Err(e) => {
                tracing::error!(error = ?e, "failed to parse request");

                let response = MessageResponseBuilder::from_message_request(request)
                    .error_msg(&request.header().clone(), ResponseCode::FormErr);

                let result = match response_handle.send_response(response).await {
                    Ok(info) => info,
                    Err(e) => {
                        tracing::error!(error = ?e, "failed to send error response");
                        ResponseInfo::from(*request.header())
                    }
                };
                self.metrics.request_duration.record(
                    start.elapsed().as_millis() as f64,
                    &[KeyValue::new("type", "parse-error")],
                );
                return result;
            }
        };

        // Log the query
        let domain_name = request_info.query.name().to_string();
        tracing::info!(
            name = %domain_name,
            query_type = ?request_info.query.query_type(),
            src = %request.src(),
            "received DNS query"
        );

        // Check blocklist
        if self.blocklist.is_blocked(&domain_name).await {
            tracing::info!(
                name = %domain_name,
                src = %request.src(),
                "blocked domain query"
            );

            let attrs = [KeyValue::new("type", "explicit-block")];
            self.metrics.responses.add(1, &attrs);

            let response = MessageResponseBuilder::from_message_request(request)
                .error_msg(&request.header().clone(), ResponseCode::Refused);

            let result = match response_handle.send_response(response).await {
                Ok(info) => info,
                Err(e) => {
                    tracing::error!(error = ?e, "failed to send blocked response");
                    ResponseInfo::from(*request.header())
                }
            };
            self.metrics
                .request_duration
                .record(start.elapsed().as_millis() as f64, &attrs);
            return result;
        }

        // Build cache key
        let cache_key = format!("{}:{:?}", domain_name, request_info.query.query_type());

        // Check cache
        if let Some(cached_response) = self.cache.get(&cache_key).await {
            tracing::debug!(
                name = %domain_name,
                src = %request.src(),
                "returning cached response"
            );

            let attrs = [KeyValue::new("type", "cache-hit")];
            self.metrics.responses.add(1, &attrs);

            // Build response from cached message with correct ID
            let mut header = *cached_response.header();
            header.set_id(request.id()); // Use current request ID, not cached ID

            let response = MessageResponseBuilder::from_message_request(request).build(
                header,
                cached_response.answers().iter(),
                cached_response.name_servers().iter(),
                &[],
                cached_response.additionals().iter(),
            );
            let result = match response_handle.send_response(response).await {
                Ok(info) => info,
                Err(e) => {
                    tracing::error!(error = ?e, "failed to send cached response");
                    ResponseInfo::from(*request.header())
                }
            };
            self.metrics
                .request_duration
                .record(start.elapsed().as_millis() as f64, &attrs);
            return result;
        }

        // Build a query message to forward to upstream
        let mut request_message = Message::new();
        request_message.set_id(request.id());
        request_message.set_message_type(MessageType::Query);
        request_message.set_op_code(request.op_code());
        request_message.set_recursion_desired(request.recursion_desired());
        request_message.add_query(request_info.query.original().clone());

        // Forward to upstream
        let upstream_start = Instant::now();
        let (response_message, response_type) = match self.upstream.resolve(&request_message).await
        {
            Ok(response) => {
                tracing::debug!(
                    src = %request.src(),
                    answer_count = response.answer_count(),
                    "upstream resolution successful"
                );

                // Cache the response with TTL from DNS records
                let ttl = ResponseCache::extract_ttl(&response);
                self.cache.insert(&cache_key, response.clone(), ttl).await;

                (response, "upstream")
            }
            Err(e) => {
                tracing::warn!(error = ?e, src = %request.src(), "upstream resolution failed");

                // Build SERVFAIL response
                let mut error_msg = Message::new();
                error_msg.set_id(request_message.id());
                error_msg.set_message_type(MessageType::Response);
                error_msg.set_op_code(OpCode::Query);
                error_msg.set_response_code(ResponseCode::ServFail);
                error_msg.add_query(request_info.query.original().clone());
                (error_msg, "upstream-error")
            }
        };
        self.metrics
            .upstream_duration
            .record(upstream_start.elapsed().as_millis() as f64, &[]);
        let attrs = [KeyValue::new("type", response_type)];
        self.metrics.responses.add(1, &attrs);

        let response = MessageResponseBuilder::from_message_request(request).build(
            *response_message.header(),
            response_message.answers().iter(),
            response_message.name_servers().iter(),
            &[],
            response_message.additionals().iter(),
        );

        let result = match response_handle.send_response(response).await {
            Ok(info) => info,
            Err(e) => {
                tracing::error!(error = ?e, "failed to send response");
                ResponseInfo::from(*request.header())
            }
        };
        self.metrics
            .request_duration
            .record(start.elapsed().as_millis() as f64, &attrs);
        result
    }
}
