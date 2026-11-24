use std::net::{Ipv4Addr, SocketAddrV4};

use axum::routing::{get, post};
use axum::{Json, Router};
use color_eyre::eyre::Result;
use foundation_http_server::Server;
use foundation_shutdown::ShutdownCoordinator;
use http::StatusCode;
use serde::Deserialize;
use serde_json::Value;
use tokio::net::TcpListener;
use tower_http::decompression::RequestDecompressionLayer;

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

async fn ingest(Json(logs): Json<Vec<Value>>) -> StatusCode {
    tracing::info!(count = logs.len(), "received logs from vector");

    StatusCode::NO_CONTENT
}

fn router() -> Router {
    Router::new()
        .route("/", get(hello))
        .route("/logs", post(ingest))
        .layer(RequestDecompressionLayer::new())
}

#[tokio::main]
async fn main() -> Result<()> {
    let config = foundation_init::run::<Configuration>()?;

    let addr = SocketAddrV4::new(config.server.host, config.server.port);
    let listener = TcpListener::bind(addr).await?;

    let server = Server::new(router(), listener);

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

    #[tokio::test]
    async fn serves_hello_world() -> Result<()> {
        let request = Request::get("/").body(Body::empty())?;
        let response = router().oneshot(request).await?;

        assert_eq!(response.status(), StatusCode::OK);

        let body = response.into_body().collect().await?.to_bytes();
        assert_eq!(&body[..], b"Hello, World!");

        Ok(())
    }

    #[tokio::test]
    async fn accepts_a_batch_of_logs() -> Result<()> {
        let body = r#"[{"message":"one"},{"message":"two"}]"#;
        let request = Request::post("/logs")
            .header("content-type", "application/json")
            .body(Body::from(body))?;
        let response = router().oneshot(request).await?;

        assert_eq!(response.status(), StatusCode::NO_CONTENT);

        Ok(())
    }

    #[tokio::test]
    async fn accepts_a_gzipped_batch_of_logs() -> Result<()> {
        let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(br#"[{"message":"one"}]"#)?;
        let body = encoder.finish()?;

        let request = Request::post("/logs")
            .header("content-type", "application/json")
            .header("content-encoding", "gzip")
            .body(Body::from(body))?;
        let response = router().oneshot(request).await?;

        assert_eq!(response.status(), StatusCode::NO_CONTENT);

        Ok(())
    }

    #[tokio::test]
    async fn rejects_a_malformed_batch() -> Result<()> {
        let request = Request::post("/logs")
            .header("content-type", "application/json")
            .body(Body::from(r#"{"message":"not an array"}"#))?;
        let response = router().oneshot(request).await?;

        assert!(response.status().is_client_error());

        Ok(())
    }
}
