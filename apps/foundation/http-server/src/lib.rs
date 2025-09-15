use std::convert::Infallible;
use std::io::Error;
use std::ops::{Deref, DerefMut};
use std::time::Duration;

use axum::Router;
use axum::body::Body;
use axum::http::{Request, Response};
use axum::response::IntoResponse;
use axum::routing::MethodRouter;
use tokio::net::TcpListener;
use tower_http::trace::{
    DefaultOnRequest, DefaultOnResponse, MakeSpan, OnRequest, OnResponse, TraceLayer,
};
use tower_service::Service;
use tracing::Span;
use tracing::field::Empty;

#[derive(Copy, Clone, Debug, Default)]
struct SpanCreator;

impl<B> MakeSpan<B> for SpanCreator {
    fn make_span(&mut self, request: &Request<B>) -> Span {
        tracing::info_span!(
            "request",
            method = %request.method(),
            uri = %request.uri(),
            version = ?request.version(),
            status = Empty,
        )
    }
}

#[derive(Clone, Debug, Default)]
struct RequestTracingFilter {
    inner: DefaultOnRequest,
}

impl<B> OnRequest<B> for RequestTracingFilter {
    fn on_request(&mut self, request: &Request<B>, span: &Span) {
        self.inner.on_request(request, span);
    }
}

#[derive(Clone, Debug, Default)]
struct ResponseTracingFilter {
    inner: DefaultOnResponse,
}

impl<B> OnResponse<B> for ResponseTracingFilter {
    fn on_response(self, response: &Response<B>, latency: Duration, span: &Span) {
        span.record("status", response.status().as_u16());

        self.inner.on_response(response, latency, span);
    }
}

pub struct Server<S = ()> {
    router: Router<S>,
}

impl<S> Server<S>
where
    S: Clone + Send + Sync + 'static,
{
    pub fn new() -> Self {
        let router = Router::new();

        Server { router }
    }

    pub fn route(self, path: &str, method_router: MethodRouter<S>) -> Self {
        let router = self.router.route(path, method_router);

        Server { router }
    }

    pub fn with_state<S2>(self, state: S) -> Server<S2> {
        let router = self.router.with_state(state);

        Server { router }
    }

    pub fn nest_service<T>(self, path: &str, service: T) -> Self
    where
        T: Service<Request<Body>, Error = Infallible> + Clone + Send + Sync + 'static,
        T::Response: IntoResponse,
        T::Future: Send + 'static,
    {
        let router = self.router.nest_service(path, service);

        Server { router }
    }
}

impl Server<()> {
    pub async fn run(self, listener: TcpListener) -> Result<(), Error> {
        let trace_layer = TraceLayer::new_for_http()
            .make_span_with(SpanCreator)
            .on_request(RequestTracingFilter::default())
            .on_response(ResponseTracingFilter::default());

        let router = self.router.layer(trace_layer);

        axum::serve(listener, router).await?;

        Ok(())
    }
}

impl Default for Server {
    fn default() -> Self {
        Self::new()
    }
}

impl<S> Deref for Server<S> {
    type Target = Router<S>;

    fn deref(&self) -> &Self::Target {
        &self.router
    }
}

impl<S> DerefMut for Server<S> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.router
    }
}

#[cfg(test)]
mod tests {
    use std::net::{Ipv4Addr, SocketAddrV4};

    use axum::extract::State;
    use axum::routing::get;
    use reqwest::Client;
    use tokio::net::TcpListener;

    use crate::Server;

    type TestResult<T> = Result<T, Box<dyn std::error::Error + Send + Sync>>;

    #[tokio::test]
    async fn can_create_servers() {
        let server: Server<()> = Server::new();

        assert!(!server.router.has_routes());
    }

    #[tokio::test]
    async fn can_add_routes_to_a_server() {
        let server: Server<()> = Server::new().route("/", get(|| async { "Hello, World!" }));

        assert!(server.router.has_routes());
    }

    #[tokio::test]
    async fn can_run_a_server() -> TestResult<()> {
        let server: Server<()> = Server::new().route("/", get(|| async { "Hello, World!" }));

        let addr = SocketAddrV4::new(Ipv4Addr::LOCALHOST, 0);
        let listener = TcpListener::bind(addr).await?;
        let local_addr = listener.local_addr()?;

        let task = tokio::spawn(async move {
            if let Err(e) = server.run(listener).await {
                eprintln!("Server error: {}", e);
            }
        });

        // make a request to the server
        let client = Client::new();
        let response = client.get(format!("http://{}", local_addr)).send().await?;

        assert!(response.status().is_success());

        task.abort();

        Ok(())
    }

    async fn stateful_handler(state: State<String>) -> String {
        state.0.clone()
    }

    #[tokio::test]
    async fn can_add_state_to_a_server() -> TestResult<()> {
        let server: Server<()> = Server::new()
            .route("/", get(stateful_handler))
            .with_state("Hello, World!".to_string());

        let addr = SocketAddrV4::new(Ipv4Addr::LOCALHOST, 0);
        let listener = TcpListener::bind(addr).await?;
        let local_addr = listener.local_addr()?;

        let task = tokio::spawn(async move {
            if let Err(e) = server.run(listener).await {
                eprintln!("Server error: {}", e);
            }
        });

        // make a request to the server
        let client = Client::new();
        let response = client.get(format!("http://{}", local_addr)).send().await?;
        let body = response.text().await?;

        assert_eq!(body, "Hello, World!");
        task.abort();

        Ok(())
    }
}
