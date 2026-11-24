use std::net::{Ipv4Addr, SocketAddrV4};

use axum::extract::State;
use axum::routing::{get, post};
use axum::{Json, Router};
use color_eyre::eyre::Result;
use foundation_http_server::Server;
use foundation_shutdown::ShutdownCoordinator;
use http::StatusCode;
use serde::Deserialize;
use tokio::net::TcpListener;
use tower_http::decompression::RequestDecompressionLayer;

use crate::store::{LogRecord, LogStore};

mod store;

#[derive(Deserialize)]
struct Configuration {
    server: ServerConfiguration,
}

#[derive(Deserialize)]
struct ServerConfiguration {
    host: Ipv4Addr,
    port: u16,
}

async fn hello() -> &'static str {
    "Hello, World!"
}

async fn ingest(State(store): State<LogStore>, Json(logs): Json<Vec<LogRecord>>) -> StatusCode {
    let count = logs.len();

    match store.insert(logs).await {
        Ok(()) => {
            tracing::info!(count, "stored logs from vector");
            StatusCode::NO_CONTENT
        }
        Err(error) => {
            tracing::error!(?error, count, "failed to store logs");
            StatusCode::INTERNAL_SERVER_ERROR
        }
    }
}

fn router(store: LogStore) -> Router {
    Router::new()
        .route("/", get(hello))
        .route("/logs", post(ingest))
        .layer(RequestDecompressionLayer::new())
        .with_state(store)
}

#[tokio::main]
async fn main() -> Result<()> {
    let config = foundation_init::run::<Configuration>()?;

    let addr = SocketAddrV4::new(config.server.host, config.server.port);
    let listener = TcpListener::bind(addr).await?;

    let store = LogStore::new()?;
    let server = Server::new(router(store), listener);

    ShutdownCoordinator::new().with_task(server).run().await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use http_body_util::BodyExt;
    use tower::ServiceExt;

    use std::io::Write;

    use color_eyre::eyre::Result;
    use flate2::Compression;
    use flate2::write::GzEncoder;

    use crate::router;
    use crate::store::LogStore;

    const RECORD: &str = r#"{
        "timestamp": "2026-11-24T10:00:00.123Z",
        "image_name": "alexanderjackson/dns-server",
        "image_tag": "20261121-0830",
        "container_name": "happy_turing",
        "container_id": "abc123",
        "container_created": "2026-11-24T09:00:00Z",
        "host": "primary",
        "stream": "stdout",
        "level": "INFO",
        "target": "dns_server::resolver",
        "message": "resolved",
        "fields": {"user": "alex", "span": {"name": "query"}}
    }"#;

    fn post_logs(body: String) -> Result<Request<Body>> {
        Ok(Request::post("/logs")
            .header("content-type", "application/json")
            .body(Body::from(body))?)
    }

    #[tokio::test]
    async fn serves_hello_world() -> Result<()> {
        let request = Request::get("/").body(Body::empty())?;
        let response = router(LogStore::new()?).oneshot(request).await?;

        assert_eq!(response.status(), StatusCode::OK);

        let body = response.into_body().collect().await?.to_bytes();
        assert_eq!(&body[..], b"Hello, World!");

        Ok(())
    }

    #[tokio::test]
    async fn accepts_a_batch_of_logs() -> Result<()> {
        let store = LogStore::new()?;
        let request = post_logs(format!("[{RECORD},{RECORD}]"))?;
        let response = router(store.clone()).oneshot(request).await?;

        assert_eq!(response.status(), StatusCode::NO_CONTENT);
        assert_eq!(store.count().await?, 2);

        Ok(())
    }

    #[tokio::test]
    async fn accepts_a_gzipped_batch_of_logs() -> Result<()> {
        let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(format!("[{RECORD}]").as_bytes())?;
        let body = encoder.finish()?;

        let request = Request::post("/logs")
            .header("content-type", "application/json")
            .header("content-encoding", "gzip")
            .body(Body::from(body))?;
        let response = router(LogStore::new()?).oneshot(request).await?;

        assert_eq!(response.status(), StatusCode::NO_CONTENT);

        Ok(())
    }

    #[tokio::test]
    async fn rejects_a_malformed_batch() -> Result<()> {
        let store = LogStore::new()?;
        let request = post_logs(r#"{"message":"not an array"}"#.to_owned())?;
        let response = router(store.clone()).oneshot(request).await?;

        assert!(response.status().is_client_error());
        assert_eq!(store.count().await?, 0);

        Ok(())
    }

    #[tokio::test]
    async fn accepts_logs_without_optional_fields() -> Result<()> {
        let store = LogStore::new()?;
        let record = r#"{
            "timestamp": "2026-11-24T10:00:00Z",
            "image_name": "caddy",
            "image_tag": "latest",
            "container_name": "web",
            "container_id": "def456",
            "container_created": "2026-11-24T09:00:00Z",
            "host": "primary",
            "stream": "stderr",
            "level": null,
            "target": null,
            "message": "plain text line",
            "fields": {}
        }"#;
        let request = post_logs(format!("[{record}]"))?;
        let response = router(store.clone()).oneshot(request).await?;

        assert_eq!(response.status(), StatusCode::NO_CONTENT);
        assert_eq!(store.count().await?, 1);

        Ok(())
    }

    #[tokio::test]
    async fn rejects_a_log_missing_required_fields() -> Result<()> {
        let store = LogStore::new()?;
        let request = post_logs(r#"[{"message":"one"}]"#.to_owned())?;
        let response = router(store.clone()).oneshot(request).await?;

        assert!(response.status().is_client_error());
        assert_eq!(store.count().await?, 0);

        Ok(())
    }
}
