use std::net::SocketAddrV4;
use std::time::Instant;

use axum::extract::rejection::JsonRejection;
use axum::extract::{Query, State};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use color_eyre::eyre::Result;
use foundation_http_server::Server;
use foundation_recurring_job::RecurringJob;
use foundation_shutdown::ShutdownCoordinator;
use foundation_templating::TemplateEngine;
use http::StatusCode;
use serde::Deserialize;
use tokio::net::TcpListener;
use tower_http::decompression::RequestDecompressionLayer;
use tower_http::services::ServeDir;

use crate::configuration::Configuration;
use crate::flush::FlushJob;
use crate::metrics::IngestMetrics;
use crate::store::{LogRecord, LogStore, QueryError};
use crate::templates::IndexContext;

mod configuration;
mod flush;
mod metrics;
mod store;
mod templates;

#[derive(Clone)]
struct ApplicationState {
    store: LogStore,
    template_engine: TemplateEngine,
    ingest_metrics: IngestMetrics,
}

#[derive(Debug, Deserialize)]
struct IndexQuery {
    q: Option<String>,
}

#[tracing::instrument(skip(store, template_engine))]
async fn index(
    State(ApplicationState {
        store,
        template_engine,
        ..
    }): State<ApplicationState>,
    Query(IndexQuery { q }): Query<IndexQuery>,
) -> Response {
    // A blank query is the form being opened, not a request to run something
    let query = q.filter(|query| !query.trim().is_empty());

    let (status, outcome) = match &query {
        None => (StatusCode::OK, None),
        Some(sql) => match store.query(sql).await {
            Ok(result) => (StatusCode::OK, Some(Ok(result))),
            Err(QueryError::Internal(error)) => {
                tracing::error!(?error, "failed to run query");
                return StatusCode::INTERNAL_SERVER_ERROR.into_response();
            }
            Err(error) => (StatusCode::BAD_REQUEST, Some(Err(error.to_string()))),
        },
    };

    let total_rows = match store.count().await {
        Ok(count) => count,
        Err(error) => {
            tracing::error!(?error, "failed to count logs");
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };

    let context = IndexContext::new(query, total_rows, outcome);

    match template_engine.render_serialized("index.tera.html", &context) {
        Ok(rendered) => (status, rendered).into_response(),
        Err(error) => {
            tracing::error!(?error, "failed to render the page");
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

async fn ingest(
    State(ApplicationState {
        store,
        ingest_metrics,
        ..
    }): State<ApplicationState>,
    logs: Result<Json<Vec<LogRecord>>, JsonRejection>,
) -> Response {
    let Json(logs) = match logs {
        Ok(logs) => logs,
        Err(rejection) => {
            ingest_metrics.invalid();
            return rejection.into_response();
        }
    };

    let count = logs.len();
    let start = Instant::now();

    match store.insert(logs).await {
        Ok(()) => {
            ingest_metrics.stored(count, start.elapsed());
            tracing::info!(count, "stored logs from vector");
            StatusCode::NO_CONTENT.into_response()
        }
        Err(error) => {
            ingest_metrics.failed(start.elapsed());
            tracing::error!(?error, count, "failed to store logs");
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

fn router(
    store: LogStore,
    template_engine: TemplateEngine,
    ingest_metrics: IngestMetrics,
) -> Router {
    Router::new()
        .route("/", get(index))
        .route("/logs", post(ingest))
        .nest_service("/assets", ServeDir::new("assets"))
        .layer(RequestDecompressionLayer::new())
        .with_state(ApplicationState {
            store,
            template_engine,
            ingest_metrics,
        })
}

#[tokio::main]
async fn main() -> Result<()> {
    let config = foundation_init::run::<Configuration>()?;

    let addr = SocketAddrV4::new(config.server.host, config.server.port);
    let listener = TcpListener::bind(addr).await?;

    let store = LogStore::with_storage(config.query, config.storage.as_ref())?;
    let template_engine = TemplateEngine::new()?;
    let ingest_metrics = IngestMetrics::new(&opentelemetry::global::meter("beaver"));
    let server = Server::new(
        router(store.clone(), template_engine, ingest_metrics),
        listener,
    );

    let mut coordinator = ShutdownCoordinator::new().with_task(server);

    let flusher = match &config.storage {
        Some(storage) => {
            let credentials = if storage.is_s3() {
                foundation_credentials::load().await?.credentials_provider()
            } else {
                None
            };

            let flusher = FlushJob::new(store, credentials);
            coordinator = coordinator.with_task(RecurringJob::new(flusher.clone()));
            Some(flusher)
        }
        None => {
            tracing::warn!("no storage is configured, so logs are only kept in memory");
            None
        }
    };

    coordinator.run().await?;

    // The server has stopped accepting logs by now, so this writes the last of them
    if let Some(flusher) = flusher {
        let flushed = flusher.flush_everything().await?;
        tracing::info!(flushed, "flushed remaining logs before shutting down");
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use axum::Router;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use foundation_templating::TemplateEngine;
    use http_body_util::BodyExt;
    use tower::ServiceExt;

    use std::io::Write;

    use color_eyre::eyre::Result;
    use flate2::Compression;
    use flate2::write::GzEncoder;

    use crate::configuration::QueryConfiguration;
    use crate::metrics::IngestMetrics;
    use crate::router;
    use crate::store::{LogRecord, LogStore};

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

    fn app(store: &LogStore) -> Result<Router> {
        Ok(router(
            store.clone(),
            TemplateEngine::new()?,
            IngestMetrics::new(&opentelemetry::global::meter("test")),
        ))
    }

    async fn get_page(store: &LogStore, uri: &str) -> Result<(StatusCode, String)> {
        let request = Request::get(uri).body(Body::empty())?;
        let response = app(store)?.oneshot(request).await?;
        let status = response.status();
        let body = response.into_body().collect().await?.to_bytes();

        Ok((status, String::from_utf8(body.to_vec())?))
    }

    fn encode(sql: &str) -> Result<String> {
        Ok(serde_urlencoded::to_string([("q", sql)])?)
    }

    #[tokio::test]
    async fn serves_the_query_form() -> Result<()> {
        let store = LogStore::new(QueryConfiguration::default())?;
        store.insert(vec![serde_json::from_str(RECORD)?]).await?;

        let (status, body) = get_page(&store, "/").await?;

        assert_eq!(status, StatusCode::OK);
        assert!(body.contains("<textarea"));
        assert!(body.contains("1 row in memory"));
        assert!(body.contains("Recent errors"));
        assert!(!body.contains("<thead"));

        Ok(())
    }

    #[tokio::test]
    async fn a_blank_query_is_not_run() -> Result<()> {
        let store = LogStore::new(QueryConfiguration::default())?;
        let (status, body) = get_page(&store, "/?q=%20%20").await?;

        assert_eq!(status, StatusCode::OK);
        assert!(!body.contains("<thead"));
        assert!(!body.contains(" ms"));

        Ok(())
    }

    #[tokio::test]
    async fn renders_the_results_of_a_query() -> Result<()> {
        let store = LogStore::new(QueryConfiguration::default())?;
        store.insert(vec![serde_json::from_str(RECORD)?]).await?;

        let sql = "SELECT message, fields->>'$.user' AS user, NULL AS missing FROM logs";
        let (status, body) = get_page(&store, &format!("/?{}", encode(sql)?)).await?;

        assert_eq!(status, StatusCode::OK);
        assert!(body.contains("1 row in"));
        assert!(body.contains(">resolved</td>"));
        assert!(body.contains(">alex</td>"));
        assert!(body.contains(">NULL</td>"));
        assert!(body.contains(">user</th>"));

        Ok(())
    }

    #[tokio::test]
    async fn keeps_the_query_in_the_form() -> Result<()> {
        let store = LogStore::new(QueryConfiguration::default())?;
        let sql = "SELECT host FROM logs WHERE host = 'a<b'";
        let (_, body) = get_page(&store, &format!("/?{}", encode(sql)?)).await?;

        assert!(body.contains("WHERE host = &#39;a&lt;b&#39;</textarea>"));

        Ok(())
    }

    #[tokio::test]
    async fn escapes_values_in_the_results() -> Result<()> {
        let store = LogStore::new(QueryConfiguration::default())?;
        let mut record: LogRecord = serde_json::from_str(RECORD)?;
        record.message = Some("<script>alert(1)</script>".to_owned());
        store.insert(vec![record]).await?;

        let (_, body) = get_page(
            &store,
            &format!("/?{}", encode("SELECT message FROM logs")?),
        )
        .await?;

        assert!(!body.contains("<script>alert(1)"));
        assert!(body.contains("&lt;script&gt;alert(1)"));

        Ok(())
    }

    #[tokio::test]
    async fn shows_an_error_for_invalid_queries() -> Result<()> {
        let store = LogStore::new(QueryConfiguration::default())?;
        let (status, body) =
            get_page(&store, &format!("/?{}", encode("SELECT nope FROM logs")?)).await?;

        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert!(body.contains("nope"));
        assert!(body.contains("<textarea"));

        Ok(())
    }

    #[tokio::test]
    async fn refuses_queries_that_modify_the_logs() -> Result<()> {
        let store = LogStore::new(QueryConfiguration::default())?;
        store.insert(vec![serde_json::from_str(RECORD)?]).await?;

        let (status, body) =
            get_page(&store, &format!("/?{}", encode("DELETE FROM logs")?)).await?;

        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert!(body.contains("only SELECT queries are allowed"));
        assert_eq!(store.count().await?, 1);

        Ok(())
    }

    #[tokio::test]
    async fn cancels_slow_queries() -> Result<()> {
        let store = LogStore::new(QueryConfiguration::default())?;
        let sql = "SELECT count(*) FROM range(100000000) a, range(100000000) b";
        let (status, body) = get_page(&store, &format!("/?{}", encode(sql)?)).await?;

        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert!(body.contains("was cancelled"));

        Ok(())
    }

    #[tokio::test]
    async fn serves_static_assets() -> Result<()> {
        let store = LogStore::new(QueryConfiguration::default())?;
        let (status, body) = get_page(&store, "/assets/script.js").await?;

        assert_eq!(status, StatusCode::OK);
        assert!(body.contains("requestSubmit"));

        Ok(())
    }

    #[tokio::test]
    async fn accepts_a_batch_of_logs() -> Result<()> {
        let store = LogStore::new(QueryConfiguration::default())?;
        let request = post_logs(format!("[{RECORD},{RECORD}]"))?;
        let response = app(&store)?.oneshot(request).await?;

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
        let response = app(&LogStore::new(QueryConfiguration::default())?)?
            .oneshot(request)
            .await?;

        assert_eq!(response.status(), StatusCode::NO_CONTENT);

        Ok(())
    }

    #[tokio::test]
    async fn rejects_a_malformed_batch() -> Result<()> {
        let store = LogStore::new(QueryConfiguration::default())?;
        let request = post_logs(r#"{"message":"not an array"}"#.to_owned())?;
        let response = app(&store)?.oneshot(request).await?;

        assert!(response.status().is_client_error());
        assert_eq!(store.count().await?, 0);

        Ok(())
    }

    #[tokio::test]
    async fn accepts_logs_without_optional_fields() -> Result<()> {
        let store = LogStore::new(QueryConfiguration::default())?;
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
        let response = app(&store)?.oneshot(request).await?;

        assert_eq!(response.status(), StatusCode::NO_CONTENT);
        assert_eq!(store.count().await?, 1);

        Ok(())
    }

    #[tokio::test]
    async fn rejects_a_log_missing_required_fields() -> Result<()> {
        let store = LogStore::new(QueryConfiguration::default())?;
        let request = post_logs(r#"[{"message":"one"}]"#.to_owned())?;
        let response = app(&store)?.oneshot(request).await?;

        assert!(response.status().is_client_error());
        assert_eq!(store.count().await?, 0);

        Ok(())
    }
}
