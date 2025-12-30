use std::future::Future;
use std::time::Duration;

use axum::Router;
use axum::http::{Request, Response};
use color_eyre::eyre::Result;
use foundation_shutdown::{CancellationToken, GracefulTask};
use tokio::net::TcpListener;
use tower_http::trace::{
    DefaultOnRequest, DefaultOnResponse, MakeSpan, OnRequest, OnResponse, TraceLayer,
};
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

pub struct Server {
    router: Router<()>,
    listener: TcpListener,
}

impl Server {
    pub fn new(router: Router<()>, listener: TcpListener) -> Self {
        Self { router, listener }
    }

    pub async fn run(self) -> Result<()> {
        let ctrl_c = async {
            tokio::signal::ctrl_c()
                .await
                .expect("failed to install Ctrl+C handler");

            tracing::info!("received ctrl+c, starting graceful shutdown");
        };

        let terminate = async {
            tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
                .expect("failed to install signal handler")
                .recv()
                .await;

            tracing::info!("received terminate signal, starting graceful shutdown");
        };

        let signal = async {
            tokio::select! {
                _ = ctrl_c => {},
                _ = terminate => {},
            }
        };

        self.run_with_graceful_shutdown(signal).await
    }

    async fn run_with_graceful_shutdown<F>(self, signal: F) -> Result<()>
    where
        F: Future<Output = ()> + Send + 'static,
    {
        let trace_layer = TraceLayer::new_for_http()
            .make_span_with(SpanCreator)
            .on_request(RequestTracingFilter::default())
            .on_response(ResponseTracingFilter::default());

        let router = self.router.layer(trace_layer);

        let addr = self.listener.local_addr()?;
        tracing::info!(%addr, "listening for incoming requests");

        axum::serve(self.listener, router)
            .with_graceful_shutdown(signal)
            .await?;

        Ok(())
    }
}

impl GracefulTask for Server {
    async fn run_until_shutdown(self, shutdown: CancellationToken) -> Result<()> {
        let signal = async move {
            shutdown.cancelled().await;
        };

        self.run_with_graceful_shutdown(signal).await
    }
}

#[cfg(test)]
mod tests {
    use std::net::{Ipv4Addr, SocketAddrV4};
    use std::sync::{Arc, RwLock};
    use std::task::{Context, Poll};

    use axum::Router;
    use axum::extract::State;
    use axum::http::Request;
    use axum::routing::get;
    use color_eyre::eyre::Result;
    use reqwest::Client;
    use tokio::net::TcpListener;
    use tower_layer::Layer;
    use tower_service::Service;

    use crate::Server;

    #[tokio::test]
    async fn can_run_a_server() -> Result<()> {
        let router = Router::new().route("/", get(|| async { "Hello, World!" }));

        let addr = SocketAddrV4::new(Ipv4Addr::LOCALHOST, 0);
        let listener = TcpListener::bind(addr).await?;
        let local_addr = listener.local_addr()?;

        let server = Server::new(router, listener);

        let task = tokio::spawn(async move {
            if let Err(e) = server.run().await {
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
    async fn can_add_state_to_a_server() -> Result<()> {
        let router = Router::new()
            .route("/", get(stateful_handler))
            .with_state("Hello, World!".to_string());

        let addr = SocketAddrV4::new(Ipv4Addr::LOCALHOST, 0);
        let listener = TcpListener::bind(addr).await?;
        let local_addr = listener.local_addr()?;

        let server = Server::new(router, listener);

        let task = tokio::spawn(async move {
            if let Err(e) = server.run().await {
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

    #[derive(Clone)]
    pub struct LogLayer {
        messages: Arc<RwLock<Vec<String>>>,
    }

    impl<S> Layer<S> for LogLayer {
        type Service = LogService<S>;

        fn layer(&self, service: S) -> Self::Service {
            LogService {
                messages: Arc::clone(&self.messages),
                service,
            }
        }
    }

    // This service implements the Log behavior
    #[derive(Clone)]
    pub struct LogService<S> {
        messages: Arc<RwLock<Vec<String>>>,
        service: S,
    }

    impl<S, B> Service<Request<B>> for LogService<S>
    where
        S: Service<Request<B>> + Clone + Send + 'static,
    {
        type Response = S::Response;
        type Error = S::Error;
        type Future = S::Future;

        fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
            self.service.poll_ready(cx)
        }

        fn call(&mut self, request: Request<B>) -> Self::Future {
            let mut writer = self.messages.write().unwrap();
            writer.push(request.uri().path().to_string());

            self.service.call(request)
        }
    }

    #[tokio::test]
    async fn can_add_layers_to_a_server() -> Result<()> {
        let buffer = Arc::new(RwLock::new(Vec::new()));
        let layer = LogLayer {
            messages: Arc::clone(&buffer),
        };

        let router = Router::new()
            .route("/{capture}", get(|| async { "Hello, World!" }))
            .layer(layer);

        let addr = SocketAddrV4::new(Ipv4Addr::LOCALHOST, 0);
        let listener = TcpListener::bind(addr).await?;
        let local_addr = listener.local_addr()?;

        let server = Server::new(router, listener);

        let task = tokio::spawn(async move {
            if let Err(e) = server.run().await {
                eprintln!("Server error: {}", e);
            }
        });

        // make a request to the server
        let client = Client::new();
        let path = "/something-here";
        let response = client
            .get(format!("http://{}{path}", local_addr))
            .send()
            .await?;

        assert!(response.status().is_success());
        assert_eq!(buffer.read().unwrap().as_slice(), [path]);

        task.abort();

        Ok(())
    }

    #[tokio::test]
    async fn can_run_with_custom_shutdown_signals() -> Result<()> {
        let (tx, mut rx) = tokio::sync::broadcast::channel::<()>(1);

        let signal = async move {
            let _ = rx.recv().await;
            tracing::info!("custom shutdown signal received, starting graceful shutdown");
        };

        let router = Router::new().route("/", get(|| async { "Hello, World!" }));

        let addr = SocketAddrV4::new(Ipv4Addr::LOCALHOST, 0);
        let listener = TcpListener::bind(addr).await?;
        let local_addr = listener.local_addr()?;

        let server = Server::new(router, listener);

        let task = tokio::spawn(async move {
            if let Err(e) = server.run_with_graceful_shutdown(signal).await {
                eprintln!("Server error: {}", e);
            }
        });

        // make a request to the server
        let client = Client::new();
        let response = client.get(format!("http://{}", local_addr)).send().await?;

        assert!(response.status().is_success());

        // shutdown the server and check we cannot call it anymore
        let _ = tx.send(());

        let client = Client::new();
        let response = client.get(format!("http://{}", local_addr)).send().await;

        assert!(response.is_err_and(|err| err.is_connect()));

        task.abort();

        Ok(())
    }
}
